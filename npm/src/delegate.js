/**
 * Delegate functionality for the probe package - used automatically by AI agents
 * @module delegate
 */

import { spawn } from 'child_process';
import { randomUUID } from 'crypto';
import { getBinaryPath, buildCliArgs } from './utils.js';
import { createMessagePreview } from './tools/common.js';

// Global delegation tracking to prevent resource exhaustion
const MAX_CONCURRENT_DELEGATIONS = parseInt(process.env.MAX_CONCURRENT_DELEGATIONS || '3', 10);
const MAX_DELEGATIONS_PER_SESSION = parseInt(process.env.MAX_DELEGATIONS_PER_SESSION || '10', 10);

// Track active delegations globally and per session
const activeDelegations = new Map(); // sessionId -> count
let globalActiveDelegations = 0;

/**
 * Decrement delegation counters when a delegation completes
 */
function decrementDelegationCounters(parentSessionId, debug = false) {
	globalActiveDelegations = Math.max(0, globalActiveDelegations - 1);

	if (parentSessionId) {
		const currentCount = activeDelegations.get(parentSessionId) || 0;
		if (currentCount > 0) {
			activeDelegations.set(parentSessionId, currentCount - 1);
		}
		// Clean up if count reaches 0
		if (activeDelegations.get(parentSessionId) === 0) {
			activeDelegations.delete(parentSessionId);
		}
	}

	if (debug) {
		console.error(`[DELEGATE] Decremented counters. Global active: ${globalActiveDelegations}`);
	}
}

/**
 * Delegate a big distinct task to a probe subagent (used automatically by AI agents)
 * 
 * This function is designed for automatic use within the agentic loop. AI agents
 * should automatically identify complex multi-part requests and break them down
 * into focused, parallel tasks using this delegation mechanism.
 * 
 * Spawns a new probe agent with a clean environment that automatically:
 * - Uses the default 'code-researcher' prompt (not inherited)
 * - Disables schema validation for simpler responses
 * - Disables mermaid validation for faster processing
 * - Limits iterations to remaining parent iterations
 * 
 * @param {Object} options - Delegate options
 * @param {string} options.task - A complete, self-contained task for the subagent. Should be specific and focused on one area of expertise.
 * @param {number} [options.timeout=300] - Timeout in seconds (default: 5 minutes)
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {number} [options.currentIteration=0] - Current tool iteration count from parent agent
 * @param {number} [options.maxIterations=30] - Maximum tool iterations allowed
 * @returns {Promise<string>} The response from the delegate agent
 */
export async function delegate({ task, timeout = 300, debug = false, currentIteration = 0, maxIterations = 30, tracer = null, parentSessionId = null }) {
	if (!task || typeof task !== 'string') {
		throw new Error('Task parameter is required and must be a string');
	}

	// Check global concurrent delegation limit
	if (globalActiveDelegations >= MAX_CONCURRENT_DELEGATIONS) {
		throw new Error(`Maximum concurrent delegations (${MAX_CONCURRENT_DELEGATIONS}) reached. Please wait for some delegations to complete.`);
	}

	// Check per-session delegation limit
	if (parentSessionId) {
		const sessionCount = activeDelegations.get(parentSessionId) || 0;
		if (sessionCount >= MAX_DELEGATIONS_PER_SESSION) {
			throw new Error(`Maximum delegations per session (${MAX_DELEGATIONS_PER_SESSION}) reached for session ${parentSessionId}`);
		}
	}

	const sessionId = randomUUID();
	const startTime = Date.now();
	
	// Calculate remaining iterations for subagent
	const remainingIterations = Math.max(1, maxIterations - currentIteration);

	// Increment delegation counters
	globalActiveDelegations++;
	if (parentSessionId) {
		const currentCount = activeDelegations.get(parentSessionId) || 0;
		activeDelegations.set(parentSessionId, currentCount + 1);
	}

	if (debug) {
		console.error(`[DELEGATE] Starting delegation session ${sessionId}`);
		console.error(`[DELEGATE] Parent session: ${parentSessionId || 'none'}`);
		console.error(`[DELEGATE] Task: ${task}`);
		console.error(`[DELEGATE] Current iteration: ${currentIteration}/${maxIterations}`);
		console.error(`[DELEGATE] Remaining iterations for subagent: ${remainingIterations}`);
		console.error(`[DELEGATE] Timeout configured: ${timeout} seconds`);
		console.error(`[DELEGATE] Global active delegations: ${globalActiveDelegations}/${MAX_CONCURRENT_DELEGATIONS}`);
		console.error(`[DELEGATE] Using clean agent environment with code-researcher prompt`);
	}

	try {
		// Get the probe binary path
		const binaryPath = await getBinaryPath();
		
		// Create the agent command with automatic subagent configuration
		const args = [
			'agent',
			'--task', task,
			'--session-id', sessionId,
			'--prompt-type', 'code-researcher',  // Automatically use default code researcher prompt
			'--no-schema-validation',            // Automatically disable schema validation
			'--no-mermaid-validation',           // Automatically disable mermaid validation
			'--max-iterations', remainingIterations.toString(),  // Automatically limit to remaining iterations
			'--no-delegate'                      // Explicitly disable delegation in subagents to prevent recursion
		];
		
		if (debug) {
			args.push('--debug');
			console.error(`[DELEGATE] Using binary at: ${binaryPath}`);
			console.error(`[DELEGATE] Command args: ${args.join(' ')}`);
		}

		// Spawn the delegate process
		return new Promise((resolve, reject) => {
			// Create delegation span for telemetry if tracer is available
			const delegationSpan = tracer ? tracer.createDelegationSpan(sessionId, task) : null;
			
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
					const preview = createMessagePreview(chunk);
					console.error(`[DELEGATE] stdout chunk received (${chunk.length} chars): ${preview}`);
				}
			});

			// Collect stderr
			process.stderr.on('data', (data) => {
				const chunk = data.toString();
				stderr += chunk;
				
				if (debug) {
					const preview = createMessagePreview(chunk);
					console.error(`[DELEGATE] stderr chunk received (${chunk.length} chars): ${preview}`);
				}
			});

			// Handle process completion
			process.on('close', (code) => {
				if (isResolved) return;
				isResolved = true;

				// Decrement delegation counters
				decrementDelegationCounters(parentSessionId, debug);

				const duration = Date.now() - startTime;

				if (debug) {
					console.error(`[DELEGATE] Process completed with code ${code} in ${duration}ms`);
					console.error(`[DELEGATE] Duration: ${(duration / 1000).toFixed(2)}s`);
					console.error(`[DELEGATE] Total stdout: ${stdout.length} chars`);
					console.error(`[DELEGATE] Total stderr: ${stderr.length} chars`);
				}

				if (code === 0) {
					// Successful delegation - return the response
					const response = stdout.trim();

					if (!response) {
						if (debug) {
							console.error(`[DELEGATE] Task completed but returned empty response for session ${sessionId}`);
						}
						reject(new Error('Delegate agent returned empty response'));
						return;
					}

					if (debug) {
						console.error(`[DELEGATE] Task completed successfully for session ${sessionId}`);
						console.error(`[DELEGATE] Response length: ${response.length} chars`);
					}

					// Record successful completion in telemetry
					if (tracer) {
						tracer.recordDelegationEvent('completed', {
							'delegation.session_id': sessionId,
							'delegation.duration_ms': duration,
							'delegation.response_length': response.length,
							'delegation.success': true
						});

						if (delegationSpan) {
							delegationSpan.setAttributes({
								'delegation.result.success': true,
								'delegation.result.response_length': response.length,
								'delegation.result.duration_ms': duration
							});
							delegationSpan.setStatus({ code: 1 }); // OK
							delegationSpan.end();
						}
					}

					resolve(response);
				} else {
					// Failed delegation
					const errorMessage = stderr.trim() || `Delegate process failed with exit code ${code}`;
					if (debug) {
						console.error(`[DELEGATE] Task failed for session ${sessionId} with code ${code}`);
						console.error(`[DELEGATE] Error message: ${errorMessage}`);
					}
					
					// Record failure in telemetry
					if (tracer) {
						tracer.recordDelegationEvent('failed', {
							'delegation.session_id': sessionId,
							'delegation.duration_ms': duration,
							'delegation.exit_code': code,
							'delegation.error_message': errorMessage,
							'delegation.success': false
						});
						
						if (delegationSpan) {
							delegationSpan.setAttributes({
								'delegation.result.success': false,
								'delegation.result.exit_code': code,
								'delegation.result.error': errorMessage,
								'delegation.result.duration_ms': duration
							});
							delegationSpan.setStatus({ code: 2, message: errorMessage }); // ERROR
							delegationSpan.end();
						}
					}
					
					reject(new Error(`Delegation failed: ${errorMessage}`));
				}
			});

			// Handle process errors
			process.on('error', (error) => {
				if (isResolved) return;
				isResolved = true;

				// Decrement delegation counters on error
				decrementDelegationCounters(parentSessionId, debug);

				const duration = Date.now() - startTime;

				if (debug) {
					console.error(`[DELEGATE] Process spawn error after ${duration}ms:`, error);
					console.error(`[DELEGATE] Session ${sessionId} failed during process creation`);
					console.error(`[DELEGATE] Error type: ${error.code || 'unknown'}`);
				}

				reject(new Error(`Failed to start delegate process: ${error.message}`));
			});

			// Handle timeout
			setTimeout(() => {
				if (isResolved) return;
				isResolved = true;

				// Decrement delegation counters on timeout
				decrementDelegationCounters(parentSessionId, debug);

				const duration = Date.now() - startTime;

				if (debug) {
					console.error(`[DELEGATE] Process timeout after ${(duration / 1000).toFixed(2)}s (limit: ${timeout}s)`);
					console.error(`[DELEGATE] Terminating session ${sessionId} due to timeout`);
					console.error(`[DELEGATE] Partial stdout: ${stdout.substring(0, 500)}${stdout.length > 500 ? '...' : ''}`);
					console.error(`[DELEGATE] Partial stderr: ${stderr.substring(0, 500)}${stderr.length > 500 ? '...' : ''}`);
				}

				// Kill the process
				process.kill('SIGTERM');

				// Give it a moment to terminate gracefully
				setTimeout(() => {
					if (!process.killed) {
						if (debug) {
							console.error(`[DELEGATE] Force killing process ${sessionId} after graceful timeout`);
						}
						process.kill('SIGKILL');
					}
				}, 5000);

				reject(new Error(`Delegation timed out after ${timeout} seconds`));
			}, timeout * 1000);
		});

	} catch (error) {
		// Decrement delegation counters on setup error
		decrementDelegationCounters(parentSessionId, debug);

		const duration = Date.now() - startTime;

		if (debug) {
			console.error(`[DELEGATE] Error in delegate function after ${duration}ms:`, error);
			console.error(`[DELEGATE] Session ${sessionId} failed during setup`);
			console.error(`[DELEGATE] Error stack: ${error.stack}`);
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