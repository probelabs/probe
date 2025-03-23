// Map to store active requests by session ID
const activeRequests = new Map();

/**
 * Register a request as active
 * @param {string} sessionId - The session ID
 * @param {Object} requestData - Data about the request (can include abort functions, etc.)
 */
export function registerRequest(sessionId, requestData) {
	if (!sessionId) {
		console.warn('Attempted to register request without session ID');
		return;
	}

	console.log(`Registering request for session: ${sessionId}`);
	activeRequests.set(sessionId, requestData);
}

/**
 * Cancel a request by session ID
 * @param {string} sessionId - The session ID
 * @returns {boolean} - Whether the cancellation was successful
 */
export function cancelRequest(sessionId) {
	if (!sessionId) {
		console.warn('Attempted to cancel request without session ID');
		return false;
	}

	const requestData = activeRequests.get(sessionId);
	if (!requestData) {
		console.warn(`No active request found for session: ${sessionId}`);
		return false;
	}

	console.log(`Cancelling request for session: ${sessionId}`);

	// Call the abort function if it exists
	if (typeof requestData.abort === 'function') {
		try {
			requestData.abort();
			console.log(`Successfully aborted request for session: ${sessionId}`);
		} catch (error) {
			console.error(`Error aborting request for session ${sessionId}:`, error);
		}
	}

	// Remove the request from the active requests map
	activeRequests.delete(sessionId);
	return true;
}

/**
 * Check if a request is active
 * @param {string} sessionId - The session ID
 * @returns {boolean} - Whether the request is active
 */
export function isRequestActive(sessionId) {
	return activeRequests.has(sessionId);
}

/**
 * Get all active requests
 * @returns {Map} - Map of all active requests
 */
export function getActiveRequests() {
	return activeRequests;
}

/**
 * Clear a request from the active requests map
 * @param {string} sessionId - The session ID
 */
export function clearRequest(sessionId) {
	if (!sessionId) {
		console.warn('Attempted to clear request without session ID');
		return;
	}

	if (activeRequests.has(sessionId)) {
		console.log(`Clearing request for session: ${sessionId}`);
		activeRequests.delete(sessionId);
	}
}