#!/usr/bin/env node

import { grep } from './src/index.js';

async function testGrep() {
	console.log('Testing grep functionality...\n');

	try {
		// Test 1: Basic search
		console.log('Test 1: Basic search for "TODO" in src directory');
		const result1 = await grep({
			pattern: 'TODO',
			paths: './src',
			lineNumbers: true
		});
		console.log('Result:');
		console.log(result1);
		console.log('\n---\n');

		// Test 2: Case-insensitive search with count
		console.log('Test 2: Count "function" occurrences (case-insensitive)');
		const result2 = await grep({
			pattern: 'function',
			paths: './src',
			ignoreCase: true,
			count: true
		});
		console.log('Result:');
		console.log(result2);
		console.log('\n---\n');

		// Test 3: Files with matches
		console.log('Test 3: Files containing "export"');
		const result3 = await grep({
			pattern: 'export',
			paths: './src',
			filesWithMatches: true
		});
		console.log('Result:');
		console.log(result3);
		console.log('\n---\n');

		console.log('✅ All grep tests passed!');
	} catch (error) {
		console.error('❌ Test failed:', error.message);
		console.error(error);
		process.exit(1);
	}
}

testGrep();
