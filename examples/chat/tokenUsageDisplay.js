import chalk from 'chalk';

/**
 * TokenUsageDisplay class to format token usage information
 */
export class TokenUsageDisplay {
	/**
	 * Format a number with commas
	 * @param {number} num Number to format
	 * @returns {string} Formatted number
	 */
	formatNumber(num) {
		return num.toLocaleString();
	}

	/**
	 * Format cache tokens
	 * @param {Object} tokens Token data
	 * @returns {Object} Formatted cache data
	 */
	formatCacheTokens(tokens = {}) {
		// Calculate total cache tokens from all providers
		const totalCacheRead = tokens.cacheRead !== undefined ? tokens.cacheRead : (((tokens.anthropic || {}).cacheRead || 0) + ((tokens.openai || {}).cachedPrompt || 0));
		const totalCacheWrite = tokens.cacheWrite !== undefined ? tokens.cacheWrite : ((tokens.anthropic || {}).cacheCreation || 0);
		const totalCache = tokens.cacheTotal !== undefined ? tokens.cacheTotal : (totalCacheRead + totalCacheWrite);

		// Return consolidated cache data
		return {
			read: this.formatNumber(totalCacheRead),
			write: this.formatNumber(totalCacheWrite),
			total: this.formatNumber(totalCache)
		};
	}

	/**
	 * Format usage data for UI display
	 * @param {Object} usage Token usage data
	 * @returns {Object} Formatted usage data
	 */
	format(usage) {
		// Ensure we have a valid context window value
		const contextWindow = usage.contextWindow || 100;

		// Ensure usage.current exists
		const current = usage.current || {};

		// Format the usage data for display
		const formatted = {
			contextWindow: this.formatNumber(contextWindow),
			current: {
				request: this.formatNumber(current.request || 0),
				response: this.formatNumber(current.response || 0),
				total: this.formatNumber(current.total || 0),
				cacheRead: this.formatNumber(current.cacheRead || 0),
				cacheWrite: this.formatNumber(current.cacheWrite || 0),
				cache: this.formatCacheTokens(current)
			},
			total: {
				request: this.formatNumber((usage.total || {}).request || 0),
				response: this.formatNumber((usage.total || {}).response || 0),
				total: this.formatNumber((usage.total || {}).total || 0),
				cacheRead: this.formatNumber((usage.total || {}).cacheRead || 0),
				cacheWrite: this.formatNumber((usage.total || {}).cacheWrite || 0),
				cache: this.formatCacheTokens(usage.total || {})
			}
		};

		return formatted;
	}
}