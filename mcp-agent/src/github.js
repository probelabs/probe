/**
 * GitHub utility functions for handling Probe failure tags
 */

const FAILURE_TAG = '<probe-failure>';
const FAILURE_TAG_CLOSE = '</probe-failure>';

/**
 * Process a response to check for failure tags and extract failure message
 * @param {string} response - The AI response text
 * @returns {Object} - Object with processedResponse and shouldFail properties
 */
export function processProbeFailure(response) {
	if (!response || typeof response !== 'string') {
		return {
			processedResponse: response,
			shouldFail: false
		};
	}

	// Check if the response contains the failure tag
	const failureStartIndex = response.indexOf(FAILURE_TAG);
	
	if (failureStartIndex === -1) {
		// No failure tag found, return original response
		return {
			processedResponse: response,
			shouldFail: false
		};
	}

	// Find the closing tag
	const failureEndIndex = response.indexOf(FAILURE_TAG_CLOSE, failureStartIndex);
	
	if (failureEndIndex === -1) {
		// Opening tag found but no closing tag, treat as failure but don't extract message
		console.error('Warning: Found opening <probe-failure> tag but no closing tag');
		return {
			processedResponse: response.replace(FAILURE_TAG, ''),
			shouldFail: true
		};
	}

	// Extract the failure message
	const failureMessageStart = failureStartIndex + FAILURE_TAG.length;
	const failureMessage = response.substring(failureMessageStart, failureEndIndex).trim();

	// Remove the entire failure tag block from the response
	const beforeFailure = response.substring(0, failureStartIndex);
	const afterFailure = response.substring(failureEndIndex + FAILURE_TAG_CLOSE.length);
	let cleanedResponse = (beforeFailure + afterFailure).trim();

	// Add the failure message at the top of the response if there's a message
	if (failureMessage) {
		cleanedResponse = `❌ **Failure:** ${failureMessage}\n\n${cleanedResponse}`;
	} else {
		cleanedResponse = `❌ **Failure detected**\n\n${cleanedResponse}`;
	}

	return {
		processedResponse: cleanedResponse,
		shouldFail: true
	};
}

/**
 * Check if a response contains a failure tag (without processing)
 * @param {string} response - The response text to check
 * @returns {boolean} - True if failure tag is present
 */
export function hasProbeFailure(response) {
	return response && typeof response === 'string' && response.includes(FAILURE_TAG);
}
