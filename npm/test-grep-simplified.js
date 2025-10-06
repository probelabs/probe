#!/usr/bin/env node

import { grep } from './src/index.js';

async function testSimplifiedGrep() {
	console.log('Testing simplified grep API...\n');

	try {
		// Test 1: Basic search (line numbers enabled by default)
		console.log('Test 1: Basic search for "export" in src directory');
		const result1 = await grep({
			pattern: 'export',
			paths: './src',
			lineNumbers: true  // This should be the default
		});
		console.log('First 5 lines:');
		console.log(result1.split('\n').slice(0, 5).join('\n'));
		console.log('\n---\n');

		// Test 2: Case-insensitive search
		console.log('Test 2: Case-insensitive search for "TODO"');
		const result2 = await grep({
			pattern: 'todo',
			paths: './src',
			ignoreCase: true,
			lineNumbers: true
		});
		console.log('Result:');
		console.log(result2);
		console.log('\n---\n');

		// Test 3: Count matches
		console.log('Test 3: Count "function" occurrences');
		const result3 = await grep({
			pattern: 'function',
			paths: './src',
			count: true
		});
		console.log('First 5 files:');
		console.log(result3.split('\n').slice(0, 5).join('\n'));
		console.log('\n---\n');

		// Test 4: Search with context
		console.log('Test 4: Search with 1 line of context');
		const result4 = await grep({
			pattern: 'export.*grep',
			paths: './src/index.js',
			context: 1,
			lineNumbers: true
		});
		console.log('Result:');
		console.log(result4);
		console.log('\n---\n');

		console.log('✅ All simplified grep tests passed!');
	} catch (error) {
		console.error('❌ Test failed:', error.message);
		console.error(error);
		process.exit(1);
	}
}

testSimplifiedGrep();
