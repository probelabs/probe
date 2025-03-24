use std::fs;

// Import the necessary functions from the extract module
use probe::extract::{
    process_file_for_extraction,
};

#[test]
fn test_javascript_extraction() {
    // Create a temporary file with Javascript code for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.js");
    let content = r#"
AFRAME.registerComponent('position', positionComponent)
const positionComponent = {
  schema: {type: 'vec3'},

  update: function () {
    var object3D = this.el.object3D;
    var data = this.data;
    object3D.position.set(data.x, data.y, data.z);
  },

  remove: function () {
    // Pretty much for mixins.
    this.el.object3D.position.set(0, 0, 0);
  }
};
"#;
    fs::write(&file_path, content).unwrap();

    
    // Test extracting a function
    let result =
        process_file_for_extraction(&file_path, Some(3), None, None, false, 0, None).unwrap();
    println!("Result: {:?}", result);
    println!("Result: {:?}", result.code);
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert!(result.lines.0 <= 3 && result.lines.1 >= 3);
    
    assert!(result.code.contains("const positionComponent = {"));
}