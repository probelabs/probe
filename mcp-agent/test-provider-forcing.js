#!/usr/bin/env node

// Simple script to test provider forcing in the mcp-agent
const { spawn } = require('child_process');
const path = require('path');

// Get the path to the index.js file
const indexPath = path.join(__dirname, 'src', 'index.js');

console.log('Testing provider forcing in mcp-agent');
console.log('=====================================');

// Test with --google flag
console.log('\nTesting with --google flag:');
const child = spawn('node', [indexPath, '--google'], {
	env: {
		...process.env,
		// Make sure we have both API keys for testing
		ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY || 'dummy-anthropic-key',
		GOOGLE_API_KEY: process.env.GOOGLE_API_KEY || 'dummy-google-key'
	}
});

// Collect stdout and stderr
let output = '';
child.stdout.on('data', (data) => {
	process.stdout.write(data);
});

child.stderr.on('data', (data) => {
	const text = data.toString();
	output += text;
	process.stderr.write(data);
});

// Handle process exit
child.on('close', (code) => {
	console.log(`\nProcess exited with code ${code}`);

	// Check if the output contains the expected messages
	const forceProviderSet = output.includes('Forcing provider: google');
	const providerForcedMessage = output.includes('Provider forced to: google');
	const usingGoogleProvider = output.includes('Using Google provider as forced');

	console.log('\nTest Results:');
	console.log(`- Force provider set: ${forceProviderSet ? '✅' : '❌'}`);
	console.log(`- Provider forced message: ${providerForcedMessage ? '✅' : '❌'}`);
	console.log(`- Using Google provider: ${usingGoogleProvider ? '✅' : '❌'}`);

	// Kill the process after 2 seconds (since it's a server that won't exit on its own)
	setTimeout(() => {
		process.exit(0);
	}, 2000);
});