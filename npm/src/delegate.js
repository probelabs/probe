/**
 * Delegate functionality for the probe package - used automatically by AI agents
 * Uses ProbeAgent SDK directly instead of spawning processes for better performance
 * @module delegate
 */

import { randomUUID } from 'crypto';
import { ProbeAgent } from './agent/ProbeAgent.js';

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
 * Creates a new ProbeAgent instance with a clean environment that automatically:
 * - Uses the default 'code-researcher' prompt (not inherited)
 * - Disables schema validation for simpler responses
 * - Disables mermaid validation for faster processing
 * - Disables delegation to prevent recursion
 * - Limits iterations to remaining parent iterations
 *
 * @param {Object} options - Delegate options
 * @param {string} options.task - A complete, self-contained task for the subagent. Should be specific and focused on one area of expertise.
 * @param {number} [options.timeout=300] - Timeout in seconds (default: 5 minutes)
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {number} [options.currentIteration=0] - Current tool iteration count from parent agent
 * @param {number} [options.maxIterations=30] - Maximum tool iterations allowed
 * @param {string} [options.parentSessionId=null] - Parent session ID for tracking
 * @param {string} [options.path] - Search directory path (inherited from parent)
 * @param {string} [options.provider] - AI provider (inherited from parent)
 * @param {string} [options.model] - AI model (inherited from parent)
 * @param {Object} [options.tracer=null] - Telemetry tracer instance
 * @returns {Promise<string>} The response from the delegate agent
 */
export async function delegate({
	task,
	timeout = 300,
	debug = false,
	currentIteration = 0,
	maxIterations = 30,
	tracer = null,
	parentSessionId = null,
	path = null,
	provider = null,
	model = null
}) {
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
		console.error(`[DELEGATE] Using ProbeAgent SDK with code-researcher prompt`);
	}

	// Create delegation span for telemetry if tracer is available
	const delegationSpan = tracer ? tracer.createDelegationSpan(sessionId, task) : null;

	try {
		// Create a new ProbeAgent instance for the delegated task
		const subagent = new ProbeAgent({
			sessionId,
			promptType: 'code-researcher',  // Clean prompt, not inherited from parent
			enableDelegate: false,           // Explicitly disable delegation to prevent recursion
			disableMermaidValidation: true,  // Faster processing
			disableJsonValidation: true,     // Simpler responses
			maxIterations: remainingIterations,
			debug,
			tracer,
			path,      // Inherit from parent
			provider,  // Inherit from parent
			model      // Inherit from parent
		});

		if (debug) {
			console.error(`[DELEGATE] Created subagent with session ${sessionId}`);
			console.error(`[DELEGATE] Subagent config: promptType=code-researcher, enableDelegate=false, maxIterations=${remainingIterations}`);
		}

		// Set up timeout
		const timeoutPromise = new Promise((_, reject) => {
			setTimeout(() => {
				reject(new Error(`Delegation timed out after ${timeout} seconds`));
			}, timeout * 1000);
		});

		// Execute the task with timeout
		const answerPromise = subagent.answer(task);
		const response = await Promise.race([answerPromise, timeoutPromise]);

		const duration = Date.now() - startTime;

		if (!response || !response.trim()) {
			throw new Error('Delegate agent returned empty response');
		}

		if (debug) {
			console.error(`[DELEGATE] Task completed successfully for session ${sessionId}`);
			console.error(`[DELEGATE] Duration: ${(duration / 1000).toFixed(2)}s`);
			console.error(`[DELEGATE] Response length: ${response.length} chars`);
		}

		// Record successful completion in telemetry
		if (tracer) {
			tracer.recordDelegationEvent('completed', {
				'delegation.session_id': sessionId,
				'delegation.parent_session_id': parentSessionId,
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

		// Decrement counters on success
		decrementDelegationCounters(parentSessionId, debug);

		return response;

	} catch (error) {
		const duration = Date.now() - startTime;

		// Decrement counters on error
		decrementDelegationCounters(parentSessionId, debug);

		if (debug) {
			console.error(`[DELEGATE] Task failed for session ${sessionId} after ${duration}ms`);
			console.error(`[DELEGATE] Error: ${error.message}`);
			console.error(`[DELEGATE] Stack: ${error.stack}`);
		}

		// Record failure in telemetry
		if (tracer) {
			tracer.recordDelegationEvent('failed', {
				'delegation.session_id': sessionId,
				'delegation.parent_session_id': parentSessionId,
				'delegation.duration_ms': duration,
				'delegation.error_message': error.message,
				'delegation.success': false
			});

			if (delegationSpan) {
				delegationSpan.setAttributes({
					'delegation.result.success': false,
					'delegation.result.error': error.message,
					'delegation.result.duration_ms': duration
				});
				delegationSpan.setStatus({ code: 2, message: error.message }); // ERROR
				delegationSpan.end();
			}
		}

		throw new Error(`Delegation failed: ${error.message}`);
	}
}


/**
 * Check if delegate functionality is available
 *
 * @returns {Promise<boolean>} True if delegate is available
 */
export async function isDelegateAvailable() {
	// Delegate is always available when using SDK-based approach
	return true;
}
