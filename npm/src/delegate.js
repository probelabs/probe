/**
 * Delegate functionality for the probe package
 * @module delegate
 */

import { spawn } from 'child_process';
import { randomUUID } from 'crypto';
import { getBinaryPath, buildCliArgs } from './utils.js';

/**
 * Delegate a task to a probe subagent
 * 
 * @param {Object} options - Delegate options
 * @param {string} options.task - The task to delegate to the subagent
 * @param {number} [options.timeout=300] - Timeout in seconds (default: 5 minutes)
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Promise<string>} The response from the delegate agent
 */
export async function delegate({ task, timeout = 300, debug = false }) {
	if (!task || typeof task !== 'string') {
		throw new Error('Task parameter is required and must be a string');
	}

	const sessionId = randomUUID();
	const startTime = Date.now();

	if (debug) {
		console.error(`[DELEGATE] Starting delegation session ${sessionId}`);
		console.error(`[DELEGATE] Task: ${task}`);
	}

	try {
		// Get the probe binary path
		const binaryPath = await getBinaryPath();
		
		if (debug) {
			console.error(`[DELEGATE] Using binary at: ${binaryPath}`);
		}

		// Create the agent command
		const args = ['agent', '--task', task, '--session-id', sessionId];
		
		if (debug) {
			args.push('--debug');
		}

		// Spawn the delegate process
		return new Promise((resolve, reject) => {
			const process = spawn(binaryPath, args, {
				stdio: ['pipe', 'pipe', 'pipe'],
				timeout: timeout * 1000
			});

			let stdout = '';
			let stderr = '';
			let isResolved = false;

			// Collect stdout
			process.stdout.on('data', (data) => {
				const chunk = data.toString();
				stdout += chunk;
				
				if (debug) {
					console.error(`[DELEGATE] stdout: ${chunk}`);
				}
			});

			// Collect stderr
			process.stderr.on('data', (data) => {
				const chunk = data.toString();
				stderr += chunk;
				
				if (debug) {
					console.error(`[DELEGATE] stderr: ${chunk}`);
				}
			});

			// Handle process completion
			process.on('close', (code) => {
				if (isResolved) return;
				isResolved = true;

				const duration = Date.now() - startTime;

				if (debug) {
					console.error(`[DELEGATE] Process completed with code ${code} in ${duration}ms`);
				}

				if (code === 0) {
					// Successful delegation - return the response
					const response = stdout.trim();
					
					if (!response) {
						reject(new Error('Delegate agent returned empty response'));
						return;
					}

					resolve(response);
				} else {
					// Failed delegation
					const errorMessage = stderr.trim() || `Delegate process failed with exit code ${code}`;
					reject(new Error(`Delegation failed: ${errorMessage}`));
				}
			});

			// Handle process errors
			process.on('error', (error) => {
				if (isResolved) return;
				isResolved = true;

				if (debug) {
					console.error(`[DELEGATE] Process error:`, error);
				}

				reject(new Error(`Failed to start delegate process: ${error.message}`));
			});

			// Handle timeout
			setTimeout(() => {
				if (isResolved) return;
				isResolved = true;

				if (debug) {
					console.error(`[DELEGATE] Timeout after ${timeout} seconds`);
				}

				// Kill the process
				process.kill('SIGTERM');
				
				// Give it a moment to terminate gracefully
				setTimeout(() => {
					if (!process.killed) {
						process.kill('SIGKILL');
					}
				}, 5000);

				reject(new Error(`Delegation timed out after ${timeout} seconds`));
			}, timeout * 1000);
		});

	} catch (error) {
		if (debug) {
			console.error(`[DELEGATE] Error in delegate function:`, error);
		}
		throw new Error(`Delegation setup failed: ${error.message}`);
	}
}

/**
 * Check if delegate functionality is available
 * 
 * @returns {Promise<boolean>} True if delegate is available
 */
export async function isDelegateAvailable() {
	try {
		const binaryPath = await getBinaryPath();
		return !!binaryPath;
	} catch (error) {
		return false;
	}
}