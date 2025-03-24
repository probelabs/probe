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

		// Compare outputs against the expected output structure
		assert_eq!(result.file, file_path.to_string_lossy().to_string());
		assert!(result.lines.0 == expected_start && result.lines.1 == expected_end, 
			"Line: {} | Expected: ({}, {}) | Actual: ({}, {})\nCode:{}", 
			line_number, expected_start, expected_end, result.lines.0, result.lines.1, result.code);
	}
}

#[test]
fn test_typescript_extraction_react() {
	/* Code provided by Facebook in their React Typescript Tutorial: https://react.dev/learn/typescript */
		let content = r#"
import { createContext, useContext, useState, useMemo } from 'react';

// This is a simpler example, but you can imagine a more complex object here
type ComplexObject = {
	kind: string
};

// The context is created with `| null` in the type, to accurately reflect the default value.
const Context = createContext<ComplexObject | null>(null);

// The `| null` will be removed via the check in the Hook.
const useGetComplexObject = () => {
	const object = useContext(Context);
	if (!object) { throw new Error("useGetComplexObject must be used within a Provider") }
	return object;
}

export default function MyApp() {
	const object = useMemo(() => ({ kind: "complex" }), []);

	return (
		<Context.Provider value={object}>
			<MyComponent />
		</Context.Provider>
	)
}

function MyComponent() {
	const object = useGetComplexObject();

	return (
		<div>
			<p>Current object: {object.kind}</p>
		</div>
	)
}
"#;

	let expected_outputs = vec![
		(1, 1, 1), // initial blank line
		(2, 2, 2), // import statement
		(3, 2, 37), // blank line -> entire module
		(4, 4, 4), // comment line
		(5, 5, 7), // type ComplexObject
		(6, 5, 7), // type ComplexObject
		(7, 5, 7), // type ComplexObject
		(8, 2, 37), // blank line -> entire module
		(9, 5, 9), // comment line  !! BUG - folds in preceding code, in preference to next line.
		(10, 10, 10), // single code line
		(11, 2, 37), // blank line -> entire module
		(12, 12, 17), // comment -> matches following function declaration
		(13, 13, 17), // useGetComplexObject function
		(14, 13, 17), // useGetComplexObject function
		(15, 13, 17), // useGetComplexObject function
		(16, 13, 17), // useGetComplexObject function
		(17, 13, 17), // useGetComplexObject function
		(18, 2, 37), // blank line -> entire module
		(19, 19, 27), // MyApp function
		(20, 20, 20), // arrow function
		(21, 19, 27), // MyApp function
		(22, 19, 27), // MyApp function
		(23, 23, 25), // <Context.Provider> JSX element
		(24, 23, 25), // <Context.Provider> JSX element !! BUG - Why doesn't this match <MyComponent /> ?
		(25, 23, 25), // <Context.Provider> JSX element
		(26, 19, 27), // MyApp function
		(27, 19, 27), // MyApp function
		(28, 2, 37), // blank line -> entire module
		(29, 29, 37), // MyComponent function
		(30, 29, 37), // MyComponent function
		(31, 29, 37), // MyComponent function
		(32, 29, 37), // MyComponent function
		(33, 33, 35), // <div> JSX element
		(34, 34, 34), // <p> JSX element
		(35, 33, 35), // <div> JSX element
		(36, 29, 37), // MyComponent function
		(37, 29, 37), // MyComponent function
	];

	execute_test(content, expected_outputs);
}





