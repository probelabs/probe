use std::fs;

// Import necessary functions from the extract module
use probe::extract::process_file_for_extraction;

fn execute_test(content: &str, expected_outputs: Vec<(usize, usize, usize)>) {

  // Create a temporary file with JavaScript code for testing
  let temp_dir = tempfile::tempdir().unwrap();
  let file_path = temp_dir.path().join("test_file.js");

  // Write the content to the temporary file
  fs::write(&file_path, content).unwrap();

  for (line_number, expected_start, expected_end) in expected_outputs {
    // Call process_file_for_extraction for the current line number
    let result = process_file_for_extraction(&file_path, Some(line_number), None, None, false, 0, None).unwrap();

    // Print or log the result for debugging
    //println!("Result for line {}: {:?}", line_number, result.code);

    // Compare outputs against the expected output structure
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert!(result.lines.0 == expected_start && result.lines.1 == expected_end, 
      "Line: {} | Expected: ({}, {}) | Actual: ({}, {})\nCode:{}", 
      line_number, expected_start, expected_end, result.lines.0, result.lines.1, result.code);
  }
}

#[test]
fn test_javascript_extraction_aframe_component() {    
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

    // Declare expected output for values 1, 2, 3, etc.
    let expected_outputs = vec![
        // This needs to be replaced with actual expected data structures
        // Example structure: (line: usize, expected_value: SomeType)
        (0, 1, 1), // before start of file
        (1, 1, 1), // initial blank line
        (2, 2, 2), // reisterComponent call
        (3, 3, 3), // const declaration
        (4, 4, 4), // schema definition
        (5, 3, 16), // entire positionComponent
        (6, 6, 10), // update function
        (7, 6, 10), // update function
        (8, 6, 10), // update function
        (9, 6, 10), // update function
        (11, 3, 16), // entire positionComponent
        (12, 12, 15), // remove function
        (13, 12, 15), // remove function
        (14, 12, 15), // remove function
        (15, 12, 15), // remove function
        (16, 16, 16), // close parentheses
        (17, 16, 16), // end of file
        (25, 16, 16), // beyond end of file
    ];

    execute_test(content, expected_outputs);
}
/* WIP, not woriking yet *
#[test]
fn test_javascript_extraction_object() {    
    let content = r#"
const user = {
	id: 1,
	name: "John Smith",
	email: "john.smith@example.com",
	profile: {
		age: 30,
		occupation: "Software Engineer",
		skills: ["JavaScript", "TypeScript", "React", "Node.js"]
	},
	isActive: true,
	lastLogin: new Date("2023-01-01")
};
"#;

    // Declare expected output for values 1, 2, 3, etc.
    let expected_outputs = vec![
        // This needs to be replaced with actual expected data structures
        // Example structure: (line: usize, expected_value: SomeType)
        (0, 1, 1), // before start of file
        (1, 1, 1), // blank line
        (2, 2, 2), // const declaaration
        (3, 2, 13), // entire object
        (4, 2, 13), // entire object
        (5, 2, 13), // entire object
        (6, 2, 13), // entire object
        (7, 2, 13), // entire object
        (8, 2, 13), // entire object
        (9, 2, 13), // entire object
        (10, 2, 13), // entire object
        (11, 2, 13), // entire object
        (12, 2, 13), // entire object
        (13, 2, 13), // entire object
    ];

    execute_test(content, expected_outputs);
}*/

