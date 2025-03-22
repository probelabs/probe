// Test script for the extract input-file functionality
import { extract } from './src/extract.js';
import { writeFileSync, unlinkSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

async function testExtractWithInputFile() {
	console.log('Testing extract with input-file option...');

	// Create a temporary file with some content that includes file paths
	const tempFilePath = join(tmpdir(), `probe-test-input-${Date.now()}.txt`);
	const content = `
Here's a reference to a file: src/extract/file_paths.rs
And another one: src/main.rs:10-20
And a symbol reference: src/query.rs#parse_query
  `;

	writeFileSync(tempFilePath, content);
	console.log(`Created temporary file: ${tempFilePath}`);

	try {
		// Test the extract function with the input file
		console.log('Calling extract with inputFile option...');
		const result = await extract({
			inputFile: tempFilePath,
			allowTests: true,
			contextLines: 5,
			format: 'plain'
		});

		console.log('Extract result:');
		console.log(result);

	} catch (error) {
		console.error('Error during extract test:', error);
	} finally {
		// Clean up
		try {
			unlinkSync(tempFilePath);
			console.log(`Removed temporary file: ${tempFilePath}`);
		} catch (err) {
			console.error(`Failed to remove temporary file: ${err.message}`);
		}
	}
}

// Run the test
testExtractWithInputFile().catch(console.error);