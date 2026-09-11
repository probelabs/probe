use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for QML (Qt Modeling Language, .qml)
///
/// QML embeds JavaScript: object definitions contain property declarations,
/// property bindings, signal declarations/handlers, and JS functions.
/// The tree-sitter-qmljs grammar exposes these as `ui_*` node kinds on top
/// of the usual JavaScript/TypeScript node kinds.
pub struct QmlLanguage;

impl Default for QmlLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl QmlLanguage {
    pub fn new() -> Self {
        QmlLanguage
    }

    /// Helper method to get the text content of a node
    fn get_node_text(&self, node: &Node, source: &[u8]) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        String::from_utf8_lossy(&source[start..end]).to_string()
    }

    /// Truncate a single-line signature to a reasonable length
    fn truncate_signature(&self, text: &str, max_len: usize) -> String {
        let trimmed = text.trim();
        if trimmed.len() > max_len {
            format!("{}...", &trimmed[..max_len - 3])
        } else {
            trimmed.to_string()
        }
    }

    /// Find the `id: <name>` binding inside a ui_object_initializer, if any
    fn find_object_id(&self, initializer: &Node, source: &[u8]) -> Option<String> {
        let mut cursor = initializer.walk();
        for child in initializer.children(&mut cursor) {
            if child.kind() == "ui_binding" {
                if let Some(name) = child.child_by_field_name("name") {
                    if name.utf8_text(source).unwrap_or("") == "id" {
                        if let Some(value) = child.child_by_field_name("value") {
                            return Some(value.utf8_text(source).unwrap_or("").to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

impl LanguageImpl for QmlLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_qmljs::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "qml"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "ui_object_definition"          // Rectangle { ... }
                | "ui_object_definition_binding" // delegate: Item { ... }
                | "ui_binding"              // property: value / onSignal: { ... }
                | "ui_property"             // property int count: 0
                | "ui_signal"               // signal activated(string id)
                | "function_declaration"    // function reload() { ... }
                | "ui_import"               // import QtQuick 2.15
                | "ui_inline_component"     // component Foo: Item { ... }
                | "ui_pragma"               // pragma Singleton
                | "comment" // // and /* */ comments
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // QML: QtTest TestCase objects are the standard test containers
        if node_type == "ui_object_definition" {
            if let Some(type_name) = node.child_by_field_name("type_name") {
                let name = type_name.utf8_text(source).unwrap_or("");
                if name == "TestCase" || name.ends_with("TestCase") {
                    if debug_mode {
                        println!("DEBUG: Test node detected (QML): TestCase object '{name}'");
                    }
                    return true;
                }
            }
        }

        // QML/JS: test functions follow the test_* naming convention (QtTest)
        if node_type == "function_declaration" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("");
                if name.starts_with("test_") || name.contains("Test") {
                    if debug_mode {
                        println!("DEBUG: Test node detected (QML): test function '{name}'");
                    }
                    return true;
                }
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "ui_object_definition" => {
                // Type name plus `id` when present: `ApplicationWindow (id: root) { ... }`
                let type_name = node
                    .child_by_field_name("type_name")
                    .map(|n| n.utf8_text(source).unwrap_or("").to_string())?;
                if let Some(init) = node.child_by_field_name("initializer") {
                    if let Some(id) = self.find_object_id(&init, source) {
                        return Some(format!("{type_name} (id: {id}) {{ ... }}"));
                    }
                }
                Some(format!("{type_name} {{ ... }}"))
            }
            "ui_object_definition_binding" => {
                // Grouped-property syntax: `NumberAnimation on x { ... }`
                let name = node
                    .child_by_field_name("name")
                    .map(|n| n.utf8_text(source).unwrap_or(""))
                    .unwrap_or("");
                let type_name = node
                    .child_by_field_name("type_name")
                    .map(|n| n.utf8_text(source).unwrap_or(""))
                    .unwrap_or("");
                Some(format!("{type_name} on {name} {{ ... }}"))
            }
            "ui_binding" => {
                // Property bindings and signal handlers: `width: 1024`, `onClicked: { ... }`,
                // object-valued bindings: `delegate: Item { ... }`
                let name = node
                    .child_by_field_name("name")
                    .map(|n| n.utf8_text(source).unwrap_or(""))
                    .unwrap_or("");
                if let Some(value) = node.child_by_field_name("value") {
                    match value.kind() {
                        "statement_block" => return Some(format!("{name}: {{ ... }}")),
                        "ui_object_definition" => {
                            let type_name = value
                                .child_by_field_name("type_name")
                                .map(|n| n.utf8_text(source).unwrap_or(""))
                                .unwrap_or("");
                            return Some(format!("{name}: {type_name} {{ ... }}"));
                        }
                        "ui_object_array" => return Some(format!("{name}: [...]")),
                        _ => {}
                    }
                    let value_text = value.utf8_text(source).unwrap_or("");
                    let first_line = value_text.lines().next().unwrap_or("");
                    return Some(format!(
                        "{name}: {}",
                        self.truncate_signature(first_line, 60)
                    ));
                }
                Some(format!("{name}:"))
            }
            "ui_property" => {
                // Full declaration line: `property string selectedId: ""`
                let text = self.get_node_text(node, source);
                Some(self.truncate_signature(text.lines().next().unwrap_or(""), 80))
            }
            "ui_signal" => {
                // `signal requirementActivated(string reqId)`
                let text = self.get_node_text(node, source);
                Some(self.truncate_signature(text.lines().next().unwrap_or(""), 80))
            }
            "function_declaration" => {
                // JS-style function signature without the body
                let name = node
                    .child_by_field_name("name")
                    .map(|n| n.utf8_text(source).unwrap_or(""))
                    .unwrap_or("");
                let params = node
                    .child_by_field_name("parameters")
                    .map(|n| n.utf8_text(source).unwrap_or(""))
                    .unwrap_or("()");
                Some(format!("function {name}{params} {{ ... }}"))
            }
            "ui_import" => {
                let text = self.get_node_text(node, source);
                Some(self.truncate_signature(text.lines().next().unwrap_or(""), 80))
            }
            "ui_inline_component" => {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| n.utf8_text(source).unwrap_or(""))
                    .unwrap_or("");
                Some(format!("component {name}: {{ ... }}"))
            }
            "ui_pragma" => {
                let text = self.get_node_text(node, source);
                Some(self.truncate_signature(text.lines().next().unwrap_or(""), 80))
            }
            "comment" => {
                let comment_text = self.get_node_text(node, source);
                Some(self.truncate_signature(&comment_text, 60))
            }
            _ => None,
        }
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut current = node;

        while let Some(parent) = current.parent() {
            // JS functions and signal handlers (onXxx bindings with a block body)
            // are the enclosing "function-like" scopes in QML
            if parent.kind() == "function_declaration" {
                return Some(parent);
            }
            if parent.kind() == "ui_binding" {
                if let Some(value) = parent.child_by_field_name("value") {
                    if value.kind() == "statement_block" {
                        return Some(parent);
                    }
                }
            }
            current = parent;
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_qml(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_qmljs::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn get_first_node_of_kind<'a>(
        node: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = get_first_node_of_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    fn get_all_nodes_of_kind<'a>(
        node: tree_sitter::Node<'a>,
        kind: &str,
        out: &mut Vec<tree_sitter::Node<'a>>,
    ) {
        if node.kind() == kind {
            out.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            get_all_nodes_of_kind(child, kind, out);
        }
    }

    const SAMPLE: &str = r#"// Main window
import QtQuick 2.15
import QtQuick.Controls 2.15

ApplicationWindow {
    id: root
    visible: true
    width: 1024

    // currently selected id
    property string selectedId: ""
    property int itemCount: 0

    signal requirementActivated(string reqId)

    function reloadRequirements() {
        itemCount = 5
    }

    function formatTitle(name, version) {
        return name + " v" + version
    }

    onSelectedIdChanged: {
        console.log("changed:", selectedId)
    }

    ListView {
        id: listView
        anchors.fill: parent

        delegate: Item {
            width: 40
        }
    }
}
"#;

    #[test]
    fn test_qml_language_creation() {
        let _lang = QmlLanguage::new();
        // Basic test to ensure QmlLanguage can be created
    }

    #[test]
    fn test_qml_get_extension() {
        let lang = QmlLanguage::new();
        #[allow(deprecated)]
        let ext = lang.get_extension();
        assert_eq!(ext, "qml");
    }

    #[test]
    fn test_acceptable_parents() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let kinds = [
            "ui_object_definition",
            "ui_binding",
            "ui_property",
            "ui_signal",
            "function_declaration",
            "ui_import",
            "comment",
        ];
        for kind in kinds {
            let node = get_first_node_of_kind(root, kind);
            assert!(node.is_some(), "expected to find node kind {kind}");
            assert!(
                lang.is_acceptable_parent(&node.unwrap()),
                "{kind} should be an acceptable parent"
            );
        }

        // delegate: Item { ... } parses as ui_binding with an object value
        let mut bindings = Vec::new();
        get_all_nodes_of_kind(root, "ui_binding", &mut bindings);
        assert!(bindings
            .iter()
            .any(|b| lang.is_acceptable_parent(b)));

        // Non-acceptable: plain identifiers/expressions
        if let Some(ident) = get_first_node_of_kind(root, "identifier") {
            assert!(!lang.is_acceptable_parent(&ident));
        }
    }

    #[test]
    fn test_object_definition_signature() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let obj = get_first_node_of_kind(root, "ui_object_definition").unwrap();
        let sig = lang.get_symbol_signature(&obj, SAMPLE.as_bytes()).unwrap();
        assert!(
            sig.contains("ApplicationWindow"),
            "signature should contain type name: {sig}"
        );
        assert!(sig.contains("id: root"), "signature should contain id: {sig}");
    }

    #[test]
    fn test_property_and_signal_signatures() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let prop = get_first_node_of_kind(root, "ui_property").unwrap();
        let sig = lang.get_symbol_signature(&prop, SAMPLE.as_bytes()).unwrap();
        assert_eq!(sig, "property string selectedId: \"\"");

        let signal = get_first_node_of_kind(root, "ui_signal").unwrap();
        let sig = lang
            .get_symbol_signature(&signal, SAMPLE.as_bytes())
            .unwrap();
        assert_eq!(sig, "signal requirementActivated(string reqId)");
    }

    #[test]
    fn test_binding_signatures() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let mut bindings = Vec::new();
        get_all_nodes_of_kind(root, "ui_binding", &mut bindings);
        assert!(!bindings.is_empty());

        // Simple value binding
        let width_binding = bindings
            .iter()
            .find(|b| {
                b.child_by_field_name("name")
                    .map(|n| n.utf8_text(SAMPLE.as_bytes()).unwrap_or(""))
                    .unwrap_or("")
                    == "width"
            })
            .copied()
            .unwrap();
        let sig = lang
            .get_symbol_signature(&width_binding, SAMPLE.as_bytes())
            .unwrap();
        assert_eq!(sig, "width: 1024");

        // Signal handler binding with block body
        let handler = bindings
            .iter()
            .find(|b| {
                b.child_by_field_name("name")
                    .map(|n| n.utf8_text(SAMPLE.as_bytes()).unwrap_or(""))
                    .unwrap_or("")
                    == "onSelectedIdChanged"
            })
            .copied()
            .unwrap();
        let sig = lang
            .get_symbol_signature(&handler, SAMPLE.as_bytes())
            .unwrap();
        assert_eq!(sig, "onSelectedIdChanged: { ... }");
    }

    #[test]
    fn test_function_signature_extraction() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let mut funcs = Vec::new();
        get_all_nodes_of_kind(root, "function_declaration", &mut funcs);
        assert_eq!(funcs.len(), 2);

        let sig = lang
            .get_symbol_signature(&funcs[0], SAMPLE.as_bytes())
            .unwrap();
        assert_eq!(sig, "function reloadRequirements() { ... }");

        let sig2 = lang
            .get_symbol_signature(&funcs[1], SAMPLE.as_bytes())
            .unwrap();
        assert_eq!(sig2, "function formatTitle(name, version) { ... }");
    }

    #[test]
    fn test_import_and_comment_signatures() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let import = get_first_node_of_kind(root, "ui_import").unwrap();
        let sig = lang
            .get_symbol_signature(&import, SAMPLE.as_bytes())
            .unwrap();
        assert!(sig.contains("import QtQuick 2.15"));

        let comment = get_first_node_of_kind(root, "comment").unwrap();
        assert!(lang.is_acceptable_parent(&comment));
        let sig = lang
            .get_symbol_signature(&comment, SAMPLE.as_bytes())
            .unwrap();
        assert!(sig.contains("Main window"));
    }

    #[test]
    fn test_nested_object_signature() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        // `delegate: Item { ... }` is a ui_binding whose value is ui_object_definition
        let mut bindings = Vec::new();
        get_all_nodes_of_kind(root, "ui_binding", &mut bindings);
        let delegate = bindings
            .iter()
            .find(|b| {
                b.child_by_field_name("name")
                    .map(|n| n.utf8_text(SAMPLE.as_bytes()).unwrap_or(""))
                    .unwrap_or("")
                    == "delegate"
            })
            .copied()
            .unwrap();
        let sig = lang
            .get_symbol_signature(&delegate, SAMPLE.as_bytes())
            .unwrap();
        assert_eq!(sig, "delegate: Item { ... }");
    }

    #[test]
    fn test_object_definition_binding_signature() {
        // `Type on property { ... }` grouped-property syntax
        let lang = QmlLanguage::new();
        let source = "Rectangle {\n    NumberAnimation on x {\n        duration: 250\n    }\n}\n";
        let tree = parse_qml(source);
        let root = tree.root_node();

        let binding = get_first_node_of_kind(root, "ui_object_definition_binding");
        assert!(binding.is_some());
        let binding = binding.unwrap();
        assert!(lang.is_acceptable_parent(&binding));
        let sig = lang.get_symbol_signature(&binding, source.as_bytes()).unwrap();
        assert_eq!(sig, "NumberAnimation on x { ... }");
    }

    #[test]
    fn test_test_node_detection() {
        let lang = QmlLanguage::new();

        // QtTest TestCase object
        let test_source = r#"import QtTest 1.15

TestCase {
    name: "MathTests"

    function test_addition() {
        compare(1 + 1, 2)
    }

    function helper() {
        return 42
    }
}
"#;
        let tree = parse_qml(test_source);
        let root = tree.root_node();

        let obj = get_first_node_of_kind(root, "ui_object_definition").unwrap();
        assert!(lang.is_test_node(&obj, test_source.as_bytes()));

        let mut funcs = Vec::new();
        get_all_nodes_of_kind(root, "function_declaration", &mut funcs);
        assert_eq!(funcs.len(), 2);
        assert!(lang.is_test_node(&funcs[0], test_source.as_bytes()));
        assert!(!lang.is_test_node(&funcs[1], test_source.as_bytes()));

        // Non-test QML should not be flagged
        let plain_tree = parse_qml(SAMPLE);
        let plain_root = plain_tree.root_node();
        let plain_obj = get_first_node_of_kind(plain_root, "ui_object_definition").unwrap();
        assert!(!lang.is_test_node(&plain_obj, SAMPLE.as_bytes()));
        let mut plain_funcs = Vec::new();
        get_all_nodes_of_kind(plain_root, "function_declaration", &mut plain_funcs);
        for f in plain_funcs {
            assert!(!lang.is_test_node(&f, SAMPLE.as_bytes()));
        }
    }

    #[test]
    fn test_find_parent_function() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        // A statement inside a function resolves to that function
        let mut assignments = Vec::new();
        get_all_nodes_of_kind(root, "assignment_expression", &mut assignments);
        let inside = assignments
            .iter()
            .find(|a| {
                lang.get_node_text(a, SAMPLE.as_bytes())
                    .contains("itemCount = 5")
            })
            .copied()
            .unwrap();
        let parent_fn = lang.find_parent_function(inside);
        assert!(parent_fn.is_some());
        assert_eq!(parent_fn.unwrap().kind(), "function_declaration");

        // A statement inside a signal handler resolves to the handler binding
        let mut calls = Vec::new();
        get_all_nodes_of_kind(root, "call_expression", &mut calls);
        let handler_call = calls
            .iter()
            .find(|c| {
                lang.get_node_text(c, SAMPLE.as_bytes())
                    .contains("console.log")
            })
            .copied()
            .unwrap();
        let parent_handler = lang.find_parent_function(handler_call);
        assert!(parent_handler.is_some());
        assert_eq!(parent_handler.unwrap().kind(), "ui_binding");
    }

    #[test]
    fn test_is_symbol_node_defaults() {
        let lang = QmlLanguage::new();
        let tree = parse_qml(SAMPLE);
        let root = tree.root_node();

        let func = get_first_node_of_kind(root, "function_declaration").unwrap();
        assert!(lang.is_symbol_node(&func));

        let signal = get_first_node_of_kind(root, "ui_signal").unwrap();
        assert!(lang.is_symbol_node(&signal));
    }
}
