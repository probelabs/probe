#!/usr/bin/env node

/**
 * CLI wrapper for the probe binary
 * 
 * This script ensures the probe binary is downloaded and then executes it with the provided arguments.
 * It's designed to be as lightweight as possible, essentially just passing through to the actual binary.
 */

import { spawn } from 'child_process';
import { getBinaryPath } from './utils.js';

/**
 * Main function
 */
async function main() {
	try {
		// Get the path to the probe binary (this will download it if needed)
		const binaryPath = await getBinaryPath();

		// Get the arguments passed to the CLI
		const args = process.argv.slice(2);

		// Spawn the probe binary with the provided arguments
		const probeProcess = spawn(binaryPath, args, {
			stdio: 'inherit' // Pipe stdin/stdout/stderr to the parent process
		});

		// Handle process exit
		probeProcess.on('close', (code) => {
			process.exit(code);
		});

		// Handle process errors
		probeProcess.on('error', (error) => {
			console.error(`Error executing probe binary: ${error.message}`);
			process.exit(1);
		});
	} catch (error) {
		console.error(`Error: ${error.message}`);
		process.exit(1);
	}
}

// Execute the main function
main().catch(error => {
	console.error(`Unexpected error: ${error.message}`);
	process.exit(1);
});