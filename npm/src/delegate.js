/**
 * Delegate functionality for the probe package - used automatically by AI agents
 * Uses ProbeAgent SDK directly instead of spawning processes for better performance
 * @module delegate
 */

import { randomUUID } from 'crypto';
import { ProbeAgent } from './agent/ProbeAgent.js';

/**
 * DelegationManager - Simple delegation tracking with proper resource management
 * Note: In single-threaded Node.js, simple counter operations are atomic within the event loop.
 * No mutex/locking needed since operations are synchronous.
 *
 * Design notes:
 * - Uses Map instead of WeakMap because sessionIds are strings (UUIDs), not objects
 * - WeakMap only accepts objects as keys, so it cannot be used for string-based session IDs
 * - Session entries are automatically cleaned up when their count reaches 0
 * - For long-running processes, periodic cleanup of stale sessions may be needed
 */
class DelegationManager {
	constructor() {
		this.maxConcurrent = parseInt(process.env.MAX_CONCURRENT_DELEGATIONS || '3', 10);
		this.maxPerSession = parseInt(process.env.MAX_DELEGATIONS_PER_SESSION || '10', 10);
		// Default queue timeout: 60 seconds. Set DELEGATION_QUEUE_TIMEOUT=0 to disable.
		this.defaultQueueTimeout = parseInt(process.env.DELEGATION_QUEUE_TIMEOUT || '60000', 10);

		// Track delegations per session with timestamp for potential TTL cleanup
		// Map<string, { count: number, lastUpdated: number }>
		this.sessionDelegations = new Map();
		this.globalActive = 0;

		// Queue for waiting delegations (FIFO)
		// Each entry: { resolve, reject, parentSessionId, queuedAt, timeoutId }
		this.waitQueue = [];

		// Start periodic cleanup of stale sessions (every 5 minutes)
		// Wrapped in try-catch to prevent interval errors from crashing the process
		this.cleanupInterval = setInterval(() => {
			try {
				this.cleanupStaleSessions();
			} catch (error) {
				console.error('[DelegationManager] Error during cleanup:', error);
			}
		}, 5 * 60 * 1000);

		// Allow Node.js to exit even if interval is active
		if (this.cleanupInterval.unref) {
			this.cleanupInterval.unref();
		}
	}

	/**
	 * Check limits and increment counters (synchronous, atomic in Node.js event loop)
	 * @param {string|null|undefined} parentSessionId - Parent session ID for tracking
	 * @returns {boolean} true if acquired, false if would need to wait
	 */
	tryAcquire(parentSessionId) {
		// Validate parentSessionId parameter
		if (parentSessionId !== null && parentSessionId !== undefined && typeof parentSessionId !== 'string') {
			throw new TypeError('parentSessionId must be a string, null, or undefined');
		}

		// Check global limit - return false instead of throwing
		if (this.globalActive >= this.maxConcurrent) {
			return false;
		}

		// Check per-session limit
		if (parentSessionId) {
			const sessionData = this.sessionDelegations.get(parentSessionId);
			const sessionCount = sessionData?.count || 0;

			if (sessionCount >= this.maxPerSession) {
				throw new Error(`Maximum delegations per session (${this.maxPerSession}) reached for session ${parentSessionId}`);
			}
		}

		// Increment counters (atomic in single-threaded Node.js)
		this._incrementCounters(parentSessionId);

		return true;
	}

	/**
	 * Internal helper to increment counters
	 * @private
	 */
	_incrementCounters(parentSessionId) {
		this.globalActive++;

		if (parentSessionId) {
			const sessionData = this.sessionDelegations.get(parentSessionId);
			if (sessionData) {
				sessionData.count++;
				sessionData.lastUpdated = Date.now();
			} else {
				this.sessionDelegations.set(parentSessionId, {
					count: 1,
					lastUpdated: Date.now()
				});
			}
		}
	}

	/**
	 * Acquire a delegation slot, waiting in queue if necessary
	 * @param {string|null|undefined} parentSessionId - Parent session ID for tracking
	 * @param {boolean} debug - Enable debug logging
	 * @param {number|null} queueTimeout - Max time to wait in queue (ms). Defaults to this.defaultQueueTimeout. Set to 0 to disable.
	 * @returns {Promise<boolean>} Resolves when slot is acquired, rejects on timeout or session limit error
	 */
	async acquire(parentSessionId, debug = false, queueTimeout = null) {
		// Use instance default if not specified
		const effectiveTimeout = queueTimeout !== null ? queueTimeout : this.defaultQueueTimeout;
		// Try immediate acquisition first
		if (this.tryAcquire(parentSessionId)) {
			return true;
		}

		// Need to wait in queue
		if (debug) {
			console.error(`[DelegationManager] Slot unavailable (${this.globalActive}/${this.maxConcurrent}), queuing... (queue size: ${this.waitQueue.length}, timeout: ${effectiveTimeout}ms)`);
		}

		// Create a promise that will be resolved when a slot becomes available
		// or rejected if session limit is exceeded or queue timeout expires
		return new Promise((resolve, reject) => {
			const entry = {
				resolve: null,  // Will be wrapped below
				reject: null,   // Will be wrapped below
				parentSessionId,
				debug,
				queuedAt: Date.now(),
				timeoutId: null
			};

			// Wrap resolve/reject to clear timeout and prevent double-settling
			let settled = false;
			entry.resolve = (value) => {
				if (settled) return;
				settled = true;
				if (entry.timeoutId) clearTimeout(entry.timeoutId);
				resolve(value);
			};
			entry.reject = (error) => {
				if (settled) return;
				settled = true;
				if (entry.timeoutId) clearTimeout(entry.timeoutId);
				reject(error);
			};

			// Set up queue timeout if enabled
			if (effectiveTimeout > 0) {
				entry.timeoutId = setTimeout(() => {
					// Remove from queue if still there
					const index = this.waitQueue.indexOf(entry);
					if (index !== -1) {
						this.waitQueue.splice(index, 1);
					}
					entry.reject(new Error(`Delegation queue timeout: waited ${effectiveTimeout}ms for an available slot`));
				}, effectiveTimeout);
			}

			this.waitQueue.push(entry);
		});
	}

	/**
	 * Decrement counters and process queue (synchronous, atomic in Node.js event loop)
	 */
	release(parentSessionId, debug = false) {
		this.globalActive = Math.max(0, this.globalActive - 1);

		if (parentSessionId) {
			const sessionData = this.sessionDelegations.get(parentSessionId);
			if (sessionData) {
				sessionData.count = Math.max(0, sessionData.count - 1);

				// Clean up if count reaches 0
				if (sessionData.count === 0) {
					this.sessionDelegations.delete(parentSessionId);
				}
			}
		}

		if (debug) {
			console.error(`[DELEGATE] Released. Global active: ${this.globalActive}, queue size: ${this.waitQueue.length}`);
		}

		// Process next item in queue if there's capacity
		this._processQueue(debug);
	}

	/**
	 * Process the wait queue - grant slot to next waiting delegation
	 * Uses setImmediate to avoid blocking the event loop when resolving promises.
	 * @private
	 */
	_processQueue(debug = false) {
		// Collect callbacks to invoke after the loop to avoid blocking the event loop
		const toResolve = [];
		const toReject = [];

		// Process queue items one at a time when slots are available
		// Items are only removed when they can be granted or must be rejected
		while (this.waitQueue.length > 0 && this.globalActive < this.maxConcurrent) {
			const next = this.waitQueue.shift();
			if (!next) break;

			const { resolve, reject, parentSessionId, queuedAt } = next;

			// Check per-session limit before granting
			if (parentSessionId) {
				const sessionData = this.sessionDelegations.get(parentSessionId);
				const sessionCount = sessionData?.count || 0;

				if (sessionCount >= this.maxPerSession) {
					// Session limit reached - reject with error (consistent with tryAcquire behavior)
					// This is a hard limit, not something that will resolve by waiting longer
					if (debug) {
						console.error(`[DelegationManager] Session limit (${this.maxPerSession}) reached for queued item, rejecting`);
					}
					toReject.push({ reject, error: new Error(`Maximum delegations per session (${this.maxPerSession}) reached for session ${parentSessionId}`) });
					// Continue to process next item in queue
					continue;
				}
			}

			// Grant the slot
			this._incrementCounters(parentSessionId);

			if (debug) {
				const waitTime = Date.now() - queuedAt;
				console.error(`[DelegationManager] Granted slot from queue (waited ${waitTime}ms). Active: ${this.globalActive}/${this.maxConcurrent}`);
			}

			toResolve.push(resolve);
		}

		// Defer promise resolutions/rejections to next tick to avoid blocking the event loop.
		// This is critical: synchronous resolve()/reject() calls in a tight loop can saturate
		// the microtask queue and prevent other async operations from proceeding.
		if (toResolve.length > 0 || toReject.length > 0) {
			setImmediate(() => {
				for (const resolve of toResolve) {
					resolve(true);
				}
				for (const { reject, error } of toReject) {
					reject(error);
				}
			});
		}
	}

	/**
	 * Get current stats for monitoring
	 */
	getStats() {
		return {
			globalActive: this.globalActive,
			maxConcurrent: this.maxConcurrent,
			maxPerSession: this.maxPerSession,
			defaultQueueTimeout: this.defaultQueueTimeout,
			sessionCount: this.sessionDelegations.size,
			queueSize: this.waitQueue.length
		};
	}

	/**
	 * Clean up stale sessions (sessions with count=0 that haven't been updated in 1 hour)
	 */
	cleanupStaleSessions() {
		const oneHourAgo = Date.now() - (60 * 60 * 1000);
		for (const [sessionId, data] of this.sessionDelegations.entries()) {
			if (data.count === 0 && data.lastUpdated < oneHourAgo) {
				this.sessionDelegations.delete(sessionId);
			}
		}
	}

	/**
	 * Cleanup all resources (for testing or shutdown)
	 */
	cleanup() {
		if (this.cleanupInterval) {
			clearInterval(this.cleanupInterval);
			this.cleanupInterval = null;
		}

		// Clear all pending queue entries and their timeouts
		for (const entry of this.waitQueue) {
			if (entry.timeoutId) {
				clearTimeout(entry.timeoutId);
			}
			// Reject pending entries so they don't hang
			if (entry.reject) {
				entry.reject(new Error('DelegationManager was cleaned up'));
			}
		}
		this.waitQueue = [];

		this.sessionDelegations.clear();
		this.globalActive = 0;
	}
}

// Default singleton instance for backward compatibility
// New code should create per-instance DelegationManager via ProbeAgent
const defaultDelegationManager = new DelegationManager();

// Export the class for per-instance usage
export { DelegationManager };

// Default delegation timeout from environment variable or 300 seconds (5 minutes)
const DEFAULT_DELEGATE_TIMEOUT = parseInt(process.env.DELEGATE_TIMEOUT, 10) || 300;

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
 * @param {number} [options.timeout] - Timeout in seconds (default: DELEGATE_TIMEOUT env var or 300)
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {number} [options.currentIteration=0] - Current tool iteration count from parent agent
 * @param {number} [options.maxIterations=30] - Maximum tool iterations allowed
 * @param {string} [options.parentSessionId=null] - Parent session ID for tracking
 * @param {string} [options.path] - Search directory path (inherited from parent)
 * @param {string[]} [options.allowedFolders] - Allowed folders (inherited from parent)
 * @param {string} [options.provider] - AI provider (inherited from parent)
 * @param {string} [options.model] - AI model (inherited from parent)
 * @param {Object} [options.tracer=null] - Telemetry tracer instance
 * @param {boolean} [options.enableBash=false] - Enable bash tool (inherited from parent)
 * @param {Object} [options.bashConfig] - Bash configuration (inherited from parent)
 * @param {string} [options.architectureFileName] - Architecture context filename to embed from repo root
 * @param {string} [options.promptType='code-researcher'] - Prompt type for the subagent
 * @param {Array<string>|null} [options.allowedTools] - Allowed tools for the subagent (null = default)
 * @param {boolean} [options.disableTools=false] - Disable all tools for the subagent
 * @param {boolean} [options.searchDelegate] - Use delegated search in the subagent
 * @param {Object|string} [options.schema] - Optional JSON schema to enforce response format
 * @param {boolean} [options.enableTasks=false] - Enable task management for the subagent (isolated instance)
 * @param {boolean} [options.enableMcp=false] - Enable MCP tool integration (inherited from parent)
 * @param {Object} [options.mcpConfig] - MCP configuration object (inherited from parent)
 * @param {string} [options.mcpConfigPath] - Path to MCP configuration file (inherited from parent)
 * @returns {Promise<string>} The response from the delegate agent
 */
export async function delegate({
	task,
	timeout = DEFAULT_DELEGATE_TIMEOUT,
	debug = false,
	currentIteration = 0,
	maxIterations = 30,
	tracer = null,
	parentSessionId = null,
	path = null,
	allowedFolders = null,
	provider = null,
	model = null,
	enableBash = false,
	bashConfig = null,
	architectureFileName = null,
	promptType = 'code-researcher',
	allowedTools = null,
	disableTools = false,
	searchDelegate = undefined,
	schema = null,
	enableTasks = false,
	enableMcp = false,
	mcpConfig = null,
	mcpConfigPath = null,
	delegationManager = null  // Optional per-instance manager, falls back to default singleton
}) {
	if (!task || typeof task !== 'string') {
		throw new Error('Task parameter is required and must be a string');
	}

	// Use provided manager or fall back to default singleton
	const manager = delegationManager || defaultDelegationManager;

	const sessionId = randomUUID();
	const startTime = Date.now();

	// Calculate remaining iterations for subagent
	const remainingIterations = Math.max(1, maxIterations - currentIteration);

	// Create delegation span for telemetry if tracer is available and has the method
	const delegationSpan = typeof tracer?.createDelegationSpan === 'function'
		? tracer.createDelegationSpan(sessionId, task)
		: null;

	let timeoutId = null;
	let acquired = false;

	try {
		// Acquire delegation slot (waits in queue if necessary)
		await manager.acquire(parentSessionId, debug);
		acquired = true;

		if (debug) {
			const stats = manager.getStats();
			console.error(`[DELEGATE] Starting delegation session ${sessionId}`);
			console.error(`[DELEGATE] Parent session: ${parentSessionId || 'none'}`);
			console.error(`[DELEGATE] Task: ${task}`);
			console.error(`[DELEGATE] Current iteration: ${currentIteration}/${maxIterations}`);
			console.error(`[DELEGATE] Remaining iterations for subagent: ${remainingIterations}`);
			console.error(`[DELEGATE] Timeout configured: ${timeout} seconds`);
			console.error(`[DELEGATE] Global active delegations: ${stats.globalActive}/${stats.maxConcurrent}, queue: ${stats.queueSize}`);
			console.error(`[DELEGATE] Using ProbeAgent SDK with ${promptType} prompt`);
		}
		// Create a new ProbeAgent instance for the delegated task
		// IMPORTANT: We pass both path and cwd set to the same value (workspace root)
		// to prevent path doubling issues. The parent's navigation context should not
		// affect the subagent's path resolution - subagents always work from workspace root.
		// Note: enableTasks creates an isolated TaskManager for the subagent.
		// Tasks do not propagate back to the parent - each subagent has its own scope.
		const subagent = new ProbeAgent({
			sessionId,
			promptType,               // Clean prompt, not inherited from parent
			enableDelegate: false,     // Explicitly disable delegation to prevent recursion
			disableMermaidValidation: true,  // Faster processing
			disableJsonValidation: true,     // Simpler responses
			maxIterations: remainingIterations,
			debug,
			tracer,
			path,       // Workspace root (from delegateTool)
			allowedFolders, // Inherit allowed folders to keep architecture context root consistent
			cwd: path,  // Explicitly set cwd to workspace root to prevent path doubling
			provider,   // Inherit from parent
			model,      // Inherit from parent
			enableBash, // Inherit from parent
			bashConfig, // Inherit from parent
			architectureFileName,
			allowedTools,
			disableTools,
			searchDelegate,
			enableTasks, // Inherit from parent (subagent gets isolated TaskManager)
			enableMcp,   // Inherit from parent (subagent creates own MCPXmlBridge)
			mcpConfig,   // Inherit from parent
			mcpConfigPath // Inherit from parent
		});

		if (debug) {
			console.error(`[DELEGATE] Created subagent with session ${sessionId}`);
			console.error(`[DELEGATE] Subagent config: promptType=${promptType}, enableDelegate=false, maxIterations=${remainingIterations}`);
		}

		// Set up timeout with proper cleanup
		// TODO: Implement AbortController support in ProbeAgent.answer() for proper cancellation
		// Current limitation: When timeout occurs, subagent.answer() continues running in background
		// This is acceptable since:
		// 1. The promise will eventually resolve/reject and be garbage collected
		// 2. The delegation slot is properly released on timeout
		// 3. The parent receives timeout error and can handle it
		// Future improvement: Add signal parameter to ProbeAgent.answer(task, [], { signal })
		const timeoutPromise = new Promise((_, reject) => {
			timeoutId = setTimeout(() => {
				reject(new Error(`Delegation timed out after ${timeout} seconds`));
			}, timeout * 1000);
		});

		// Execute the task with timeout
		const answerOptions = schema ? { schema } : undefined;
		const answerPromise = answerOptions ? subagent.answer(task, [], answerOptions) : subagent.answer(task);
		const response = await Promise.race([answerPromise, timeoutPromise]);

		// Clear timeout immediately after race completes to prevent memory leak
		// Note: timeoutId is always set by this point (synchronous in Promise constructor)
		// but we keep the null check for defensive programming
		if (timeoutId !== null) {
			clearTimeout(timeoutId);
			timeoutId = null;
		}

		const duration = Date.now() - startTime;

		// Validate response (check for type first, then content)
		if (typeof response !== 'string') {
			throw new Error('Delegate agent returned invalid response (not a string)');
		}

		const trimmedResponse = response.trim();
		if (trimmedResponse.length === 0) {
			throw new Error('Delegate agent returned empty or whitespace-only response');
		}

		// Check for null bytes (edge case)
		if (trimmedResponse.includes('\0')) {
			throw new Error('Delegate agent returned response containing null bytes');
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

		// Release delegation slot
		if (acquired) {
			manager.release(parentSessionId, debug);
		}

		return response;

	} catch (error) {
		// Clear timeout if still active
		if (timeoutId !== null) {
			clearTimeout(timeoutId);
			timeoutId = null;
		}

		const duration = Date.now() - startTime;

		// Release delegation slot on error (only if it was acquired)
		if (acquired) {
			manager.release(parentSessionId, debug);
		}

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

/**
 * Get delegation statistics (for monitoring/debugging)
 * Note: Returns stats from the default singleton manager.
 * For per-instance stats, use manager.getStats() directly.
 *
 * @returns {Object} Current delegation stats
 */
export function getDelegationStats() {
	return defaultDelegationManager.getStats();
}

/**
 * Cleanup delegation manager (for testing or shutdown)
 * Note: Cleans up the default singleton manager.
 * For per-instance cleanup, use manager.cleanup() directly.
 */
export async function cleanupDelegationManager() {
	return defaultDelegationManager.cleanup();
}
