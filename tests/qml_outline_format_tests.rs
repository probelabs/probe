use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_qml_outline_basic_objects_and_functions() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("MainWindow.qml");

    let content = r#"// Main application window
import QtQuick 2.15
import QtQuick.Controls 2.15

ApplicationWindow {
    id: root
    visible: true
    width: 1024

    property string selectedId: ""
    property int itemCount: 0

    signal requirementActivated(string reqId)

    // Reload requirements from disk
    function reloadRequirements() {
        var items = backend.loadAll()
        itemCount = items.length
    }

    function formatTitle(name, version) {
        return name + " v" + version
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "reloadRequirements",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify QML symbols are found in outline format
    assert!(
        output.contains("reloadRequirements"),
        "Missing reloadRequirements function - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:") || output.contains("File:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_qml_outline_properties_signals_bindings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Controls.qml");

    let content = r#"import QtQuick 2.15

Item {
    id: controls

    property alias model: listView.model
    readonly property bool hasSelection: selectedId !== ""

    signal itemSelected(string itemId)

    onItemSelected: {
        console.log("selected:", itemId)
    }

    ListView {
        id: listView
        anchors.fill: parent

        delegate: Rectangle {
            width: ListView.view.width
            height: 40
            color: "transparent"
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Signal handler search
    let output = ctx.run_probe(&[
        "search",
        "itemSelected",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("itemSelected"),
        "Should find signal declaration/handler - output: {}",
        output
    );

    // Property binding search
    let output = ctx.run_probe(&[
        "search",
        "hasSelection",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("hasSelection"),
        "Should find property declaration - output: {}",
        output
    );

    // Nested object search
    let output = ctx.run_probe(&[
        "search",
        "transparent",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("Rectangle") || output.contains("transparent"),
        "Should find nested delegate object - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_qml_outline_imports_and_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Imports.qml");

    let content = r#"// Module imports for the settings page
import QtQuick 2.15
import QtQuick.Layouts 1.15
import "./components" as Components

Components.SettingsPage {
    title: qsTr("Settings")
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    let output = ctx.run_probe(&[
        "search",
        "QtQuick.Layouts",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("QtQuick.Layouts"),
        "Should find import statement - output: {}",
        output
    );

    let output = ctx.run_probe(&[
        "search",
        "settings page",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("settings page"),
        "Should find comment content - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_qml_extract_function_by_line() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Utils.qml");

    let content = r#"import QtQuick 2.15

Item {
    function formatName(first, last) {
        return first + " " + last
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    // Extract the block at line 5 (inside formatName)
    let output = ctx.run_probe(&["extract", &format!("{}:5", test_file.to_str().unwrap())])?;

    assert!(
        output.contains("formatName"),
        "Should extract the formatName function - output: {}",
        output
    );
    assert!(
        output.contains("first + \" \" + last"),
        "Should extract the function body - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_qml_testcase_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("MathTests.qml");

    let content = r#"import QtQuick 2.15
import QtTest 1.15

TestCase {
    name: "MathTests"

    function test_addition() {
        compare(1 + 1, 2)
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // With --allow-tests, TestCase contents should be searchable
    let output = ctx.run_probe(&[
        "search",
        "compare",
        test_file.to_str().unwrap(),
        "--allow-tests",
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("test_addition") || output.contains("TestCase"),
        "Should find test_addition function with --allow-tests - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_qml_search_json_symbol_signature() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("SignatureCheck.qml");

    let content = r#"import QtQuick 2.15

Item {
    id: root

    function reloadRequirements() {
        root.refreshModel()
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "refreshModel",
        test_file.to_str().unwrap(),
        "--format",
        "json",
    ])?;

    // Search JSON must carry the QML symbol signature, not null
    assert!(
        output.contains("\"symbol_signature\": \"function reloadRequirements() { ... }\""),
        "QML search JSON should include a non-null symbol_signature - output: {}",
        output
    );

    Ok(())
}
