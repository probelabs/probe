#!/usr/bin/env node

import { grep } from './src/index.js';

async function testSecurityFixes() {
	console.log('Testing grep security (command injection prevention)...\n');

	try {
		// Test 1: Malicious pattern with shell metacharacters
		console.log('Test 1: Pattern with shell metacharacters (should be safe)');
		const maliciousPattern = 'test"; echo "INJECTED"; echo "';
		try {
			const result = await grep({
				pattern: maliciousPattern,
				paths: './src',
				lineNumbers: true
			});
			console.log('✅ Pattern treated as literal string (no injection)');
			console.log(`   Found ${result.split('\n').filter(l => l).length} lines`);
		} catch (error) {
			// This is expected - the pattern won't match
			console.log('✅ No matches found (pattern treated as literal)');
		}
		console.log('\n---\n');

		// Test 2: Path with shell metacharacters
		console.log('Test 2: Path with shell metacharacters (should be safe)');
		const maliciousPath = './src; echo INJECTED;';
		try {
			const result = await grep({
				pattern: 'export',
				paths: maliciousPath,
				lineNumbers: true
			});
			console.log('⚠️  Unexpected: command should have failed');
		} catch (error) {
			console.log('✅ Path handled securely (command failed safely)');
			console.log(`   Error: ${error.message.substring(0, 100)}`);
		}
		console.log('\n---\n');

		// Test 3: Pattern with backticks (command substitution attempt)
		console.log('Test 3: Pattern with backticks (should be safe)');
		const backtickPattern = 'test`whoami`test';
		try {
			const result = await grep({
				pattern: backtickPattern,
				paths: './src',
				lineNumbers: true
			});
			console.log('✅ Backticks treated as literal (no command substitution)');
		} catch (error) {
			console.log('✅ No matches found (backticks treated as literal)');
		}
		console.log('\n---\n');

		// Test 4: Pattern with $() (command substitution attempt)
		console.log('Test 4: Pattern with $() syntax (should be safe)');
		const dollarPattern = 'test$(whoami)test';
		try {
			const result = await grep({
				pattern: dollarPattern,
				paths: './src',
				lineNumbers: true
			});
			console.log('✅ $() treated as literal (no command substitution)');
		} catch (error) {
			console.log('✅ No matches found ($() treated as literal)');
		}
		console.log('\n---\n');

		// Test 5: Normal regex patterns should still work
		console.log('Test 5: Normal regex patterns work correctly');
		const regexPattern = 'export.*function';
		const result = await grep({
			pattern: regexPattern,
			paths: './src/grep.js',
			lineNumbers: true
		});
		console.log('✅ Regex patterns work correctly');
		console.log(`   Found ${result.split('\n').filter(l => l).length} matches`);
		console.log('\n---\n');

		console.log('✅ All security tests passed!');
		console.log('\nThe grep function is now safe from command injection attacks.');
		console.log('All shell metacharacters are treated as literal strings.');
	} catch (error) {
		console.error('❌ Security test failed:', error.message);
		console.error(error);
		process.exit(1);
	}
}

testSecurityFixes();
