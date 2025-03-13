/**
 * Test script for the search functionality
 */

import { search } from './src/search.js';
import { query } from './src/query.js';
import { extract } from './src/extract.js';

async function testSearch() {
	console.log('Testing search functionality...');

	try {
		// Test regular search
		const searchResult = await search({
			path: '.',
			query: 'search',  // More likely to find matches in this codebase
			maxResults: 10
		});

		console.log('\nSearch completed successfully.');
		console.log('First few lines of results:');
		console.log(searchResult.split('\n').slice(0, 5).join('\n'));

		// Test query
		console.log('\n\nTesting query functionality...');
		const queryResult = await query({
			path: '.',
			pattern: '(variable_declaration)',  // More likely to match in JavaScript files
			language: 'javascript',
			maxResults: 5
		});

		console.log('\nQuery completed successfully.');
		console.log('First few lines of results:');
		console.log(queryResult.split('\n').slice(0, 5).join('\n'));

		// Test extract
		console.log('\n\nTesting extract functionality...');
		const extractResult = await extract({
			files: ['./src/search.js'],
			contextLines: 3
		});

		console.log('\nExtract completed successfully.');
		console.log('First few lines of results:');
		console.log(extractResult.split('\n').slice(0, 5).join('\n'));

	} catch (error) {
		console.error('Error during test:', error);
	}
}

testSearch().catch(console.error);