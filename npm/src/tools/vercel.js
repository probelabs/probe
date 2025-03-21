/**
 * Tools for Vercel AI SDK
 * @module tools/vercel
 */

import { tool } from 'ai';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { searchSchema, querySchema, extractSchema, searchDescription, queryDescription, extractDescription } from './common.js';

/**
 * Search tool generator
 * 
 * @param {Object} [options] - Configuration options
 * @param {string} [options.sessionId] - Session ID for caching search results
 * @param {number} [options.maxTokens=10000] - Default max tokens
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Object} Configured search tool
 */
export const searchTool = (options = {}) => {
	const { sessionId, maxTokens = 10000, debug = false } = options;

	return tool({
		name: 'search',
		description: searchDescription,
		parameters: searchSchema,
		execute: async ({ query: searchQuery, path, exact, allow_tests, maxTokens: paramMaxTokens }) => {
			try {
				// Use parameter maxTokens if provided, otherwise use the default
				const effectiveMaxTokens = paramMaxTokens || maxTokens;

				if (debug) {
					console.error(`Executing search with query: "${searchQuery}", path: "${path || '.'}", session: ${sessionId || 'none'}`);
				}

				const results = await search({
					query: searchQuery,
					path,
					exact,
					allow_tests,
					json: false,
					maxTokens: effectiveMaxTokens,
					session: sessionId // Pass session ID if provided
				});

				return results;
			} catch (error) {
				console.error('Error executing search command:', error);
				return `Error executing search command: ${error.message}`;
			}
		}
	});
};

/**
 * Query tool generator
 * 
 * @param {Object} [options] - Configuration options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Object} Configured query tool
 */
export const queryTool = (options = {}) => {
	const { debug = false } = options;

	return tool({
		name: 'query',
		description: queryDescription,
		parameters: querySchema,
		execute: async ({ pattern, path, language, allow_tests }) => {
			try {
				if (debug) {
					console.error(`Executing query with pattern: "${pattern}", path: "${path || '.'}", language: ${language || 'auto'}`);
				}

				const results = await query({
					pattern,
					path,
					language,
					allow_tests,
					json: false
				});

				return results;
			} catch (error) {
				console.error('Error executing query command:', error);
				return `Error executing query command: ${error.message}`;
			}
		}
	});
};

/**
 * Extract tool generator
 * 
 * @param {Object} [options] - Configuration options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Object} Configured extract tool
 */
export const extractTool = (options = {}) => {
	const { debug = false } = options;

	return tool({
		name: 'extract',
		description: extractDescription,
		parameters: extractSchema,
		execute: async ({ file_path, line, end_line, allow_tests, context_lines, format }) => {
			try {
				if (debug) {
					console.error(`Executing extract with file: "${file_path}", context lines: ${context_lines || 10}`);
				}

				// Parse file_path to handle line numbers and symbol names
				const files = [file_path];

				const results = await extract({
					files,
					allowTests: allow_tests,
					contextLines: context_lines,
					format
				});

				return results;
			} catch (error) {
				console.error('Error executing extract command:', error);
				return `Error executing extract command: ${error.message}`;
			}
		}
	});
};