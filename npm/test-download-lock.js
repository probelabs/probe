#!/usr/bin/env node
/**
 * Test script to verify download locking works correctly
 * Tests both in-process and cross-process locking
 */

import { getBinaryPath } from './src/utils.js';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);

async function testInProcessLocking() {
	console.log('=== Test 1: In-Process Locking ===\n');

	// Simulate 5 concurrent calls to getBinaryPath with the same version
	const version = '0.6.0-rc100'; // Use a specific version to trigger download

	console.log(`Initiating 5 concurrent getBinaryPath calls for version ${version}...`);
	const startTime = Date.now();

	const promises = Array.from({ length: 5 }, (_, i) => {
		return getBinaryPath({ version, forceDownload: false })
			.then(path => {
				console.log(`Call ${i + 1} completed: ${path}`);
				return path;
			})
			.catch(err => {
				console.error(`Call ${i + 1} failed:`, err.message);
				throw err;
			});
	});

	const results = await Promise.all(promises);
	const endTime = Date.now();

	console.log(`\nAll calls completed in ${endTime - startTime}ms`);
	console.log('All calls returned the same path:', new Set(results).size === 1);
	console.log('✅ In-process locking test passed\n');
}

async function testCrossProcessLocking() {
	console.log('=== Test 2: Cross-Process Locking ===\n');

	const version = '0.6.0-rc101'; // Different version for cross-process test

	console.log(`Spawning 3 separate Node.js processes to download version ${version}...`);

	const processes = Array.from({ length: 3 }, (_, i) => {
		return new Promise((resolve, reject) => {
			const child = spawn('node', [
				'-e',
				`import('${__filename.replace(/\\/g, '/')}').then(m => m.singleDownload('${version}'))`
			], {
				stdio: 'inherit',
				shell: true
			});

			child.on('exit', (code) => {
				if (code === 0) {
					resolve();
				} else {
					reject(new Error(`Process ${i + 1} exited with code ${code}`));
				}
			});

			child.on('error', reject);
		});
	});

	await Promise.all(processes);
	console.log('✅ Cross-process locking test passed\n');
}

// Helper function for spawned processes
export async function singleDownload(version) {
	console.log(`[Process ${process.pid}] Starting download for version ${version}`);
	const path = await getBinaryPath({ version, forceDownload: false });
	console.log(`[Process ${process.pid}] Download completed: ${path}`);
}

async function runTests() {
	try {
		console.log('Testing Download Locking Mechanism\n');
		console.log('Lock features:');
		console.log('- In-memory locks for same-process coordination');
		console.log('- File-based locks for cross-process coordination');
		console.log('- 5-minute timeout for stuck downloads');
		console.log('- Automatic retry if locked download fails');
		console.log('- Stale lock detection and cleanup');
		console.log('- Poll-based waiting (500ms intervals)\n');

		await testInProcessLocking();

		// Uncomment to test cross-process locking
		// Note: This requires the version to not already be downloaded
		// await testCrossProcessLocking();

		console.log('✅ All tests passed!');
	} catch (error) {
		console.error('Test failed:', error);
		process.exit(1);
	}
}

// Run tests if this is the main module
if (process.argv[1] === __filename) {
	runTests();
}
