/**
 * Tools for Vercel AI SDK
 * @module tools/vercel
 */

import { tool } from 'ai';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { delegate } from '../delegate.js';
import { searchSchema, querySchema, extractSchema, delegateSchema, searchDescription, queryDescription, extractDescription, delegateDescription, parseTargets } from './common.js';

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
	const { sessionId, maxTokens = 10000, debug = false, outline = false } = options;

	return tool({
		name: 'search',
		description: searchDescription,
		inputSchema: searchSchema,
		execute: async ({ query: searchQuery, path, allow_tests, exact, maxTokens: paramMaxTokens, language }) => {
			try {
				// Use parameter maxTokens if provided, otherwise use the default
				const effectiveMaxTokens = paramMaxTokens || maxTokens;

				// Use the path from parameters if provided, otherwise use defaultPath from config
				let searchPath = path || options.defaultPath || '.';

				// If path is "." or "./", use the defaultPath if available
				if ((searchPath === "." || searchPath === "./") && options.defaultPath) {
					if (debug) {
						console.error(`Using default path "${options.defaultPath}" instead of "${searchPath}"`);
					}
					searchPath = options.defaultPath;
				}

				if (debug) {
					console.error(`Executing search with query: "${searchQuery}", path: "${searchPath}", exact: ${exact ? 'true' : 'false'}, language: ${language || 'all'}, session: ${sessionId || 'none'}`);
				}

				const searchOptions = {
					query: searchQuery,
					path: searchPath,
					allowTests: allow_tests,
					exact,
					json: false,
					maxTokens: effectiveMaxTokens,
					session: sessionId, // Pass session ID if provided
					language // Pass language parameter if provided
				};

				// Add outline format if enabled
				if (outline) {
					searchOptions.format = 'outline-xml';
				}

				const results = await search(searchOptions);

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
		inputSchema: querySchema,
		execute: async ({ pattern, path, language, allow_tests }) => {
			try {
				// Use the path from parameters if provided, otherwise use defaultPath from config
				let queryPath = path || options.defaultPath || '.';

				// If path is "." or "./", use the defaultPath if available
				if ((queryPath === "." || queryPath === "./") && options.defaultPath) {
					if (debug) {
						console.error(`Using default path "${options.defaultPath}" instead of "${queryPath}"`);
					}
					queryPath = options.defaultPath;
				}

				if (debug) {
					console.error(`Executing query with pattern: "${pattern}", path: "${queryPath}", language: ${language || 'auto'}`);
				}

				const results = await query({
					pattern,
					path: queryPath,
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
	const { debug = false, outline = false } = options;

	return tool({
		name: 'extract',
		description: extractDescription,
		inputSchema: extractSchema,
		execute: async ({ targets, input_content, line, end_line, allow_tests, context_lines, format }) => {
			try {
				// Use the defaultPath from config for context
				let extractPath = options.defaultPath || '.';

				// If path is "." or "./", use the defaultPath if available
				if ((extractPath === "." || extractPath === "./") && options.defaultPath) {
					if (debug) {
						console.error(`Using default path "${options.defaultPath}" instead of "${extractPath}"`);
					}
					extractPath = options.defaultPath;
				}

				if (debug) {
					if (targets) {
						console.error(`Executing extract with targets: "${targets}", path: "${extractPath}", context lines: ${context_lines || 10}`);
					} else if (input_content) {
						console.error(`Executing extract with input content, path: "${extractPath}", context lines: ${context_lines || 10}`);
					}
				}

				// Create a temporary file for input content if provided
				let tempFilePath = null;
				let extractOptions = { path: extractPath };

				if (input_content) {
					// Import required modules
					const { writeFileSync, unlinkSync } = await import('fs');
					const { join } = await import('path');
					const { tmpdir } = await import('os');
					const { randomUUID } = await import('crypto');

					// Create a temporary file with the input content
					tempFilePath = join(tmpdir(), `probe-extract-${randomUUID()}.txt`);
					writeFileSync(tempFilePath, input_content);

					if (debug) {
						console.error(`Created temporary file for input content: ${tempFilePath}`);
					}

					// Apply format mapping for outline-xml to xml
					let effectiveFormat = format;
					if (outline && format === 'outline-xml') {
						effectiveFormat = 'xml';
					}

					// Set up extract options with input file
					extractOptions = {
						inputFile: tempFilePath,
						allowTests: allow_tests,
						contextLines: context_lines,
						format: effectiveFormat
					};
				} else if (targets) {
					// Parse targets to handle line numbers and symbol names
					// Split on whitespace to support multiple targets in one call
					const files = parseTargets(targets);

					// Apply format mapping for outline-xml to xml
					let effectiveFormat = format;
					if (outline && format === 'outline-xml') {
						effectiveFormat = 'xml';
					}

					// Set up extract options with files
					extractOptions = {
						files,
						allowTests: allow_tests,
						contextLines: context_lines,
						format: effectiveFormat
					};
				} else {
					throw new Error('Either targets or input_content must be provided');
				}

				// Execute the extract command
				const results = await extract(extractOptions);

				// Clean up temporary file if created
				if (tempFilePath) {
					const { unlinkSync } = await import('fs');
					try {
						unlinkSync(tempFilePath);
						if (debug) {
							console.error(`Removed temporary file: ${tempFilePath}`);
						}
					} catch (cleanupError) {
						console.error(`Warning: Failed to remove temporary file: ${cleanupError.message}`);
					}
				}

				return results;
			} catch (error) {
				console.error('Error executing extract command:', error);
				return `Error executing extract command: ${error.message}`;
			}
		}
	});
};

/**
 * Delegate tool generator
 *
 * @param {Object} [options] - Configuration options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {number} [options.timeout=300] - Default timeout in seconds
 * @param {string} [options.defaultPath] - Default path to use if not specified in call
 * @param {string[]} [options.allowedFolders] - Allowed folders for workspace isolation
 * @returns {Object} Configured delegate tool
 */
export const delegateTool = (options = {}) => {
	const { debug = false, timeout = 300, defaultPath, allowedFolders } = options;

	return tool({
		name: 'delegate',
		description: delegateDescription,
		inputSchema: delegateSchema,
		execute: async ({ task, currentIteration, maxIterations, parentSessionId, path, provider, model, tracer }) => {
			// Validate required parameters - throw errors for consistency
			if (!task || typeof task !== 'string') {
				throw new Error('Task parameter is required and must be a non-empty string');
			}

			if (task.trim().length === 0) {
				throw new Error('Task parameter cannot be empty or whitespace only');
			}

			// Validate optional numeric parameters
			if (currentIteration !== undefined && (typeof currentIteration !== 'number' || currentIteration < 0)) {
				throw new Error('currentIteration must be a non-negative number');
			}

			if (maxIterations !== undefined && (typeof maxIterations !== 'number' || maxIterations < 1)) {
				throw new Error('maxIterations must be a positive number');
			}

			// Validate optional string parameters for type consistency
			if (parentSessionId !== undefined && parentSessionId !== null && typeof parentSessionId !== 'string') {
				throw new TypeError('parentSessionId must be a string, null, or undefined');
			}

			if (path !== undefined && path !== null && typeof path !== 'string') {
				throw new TypeError('path must be a string, null, or undefined');
			}

			if (provider !== undefined && provider !== null && typeof provider !== 'string') {
				throw new TypeError('provider must be a string, null, or undefined');
			}

			if (model !== undefined && model !== null && typeof model !== 'string') {
				throw new TypeError('model must be a string, null, or undefined');
			}

			// Use inherited path if not specified in AI call
			// Priority: explicit path > defaultPath > first allowedFolder
			const effectivePath = path || defaultPath || (allowedFolders && allowedFolders[0]);

			if (debug) {
				console.error(`Executing delegate with task: "${task.substring(0, 100)}${task.length > 100 ? '...' : ''}"`);
				if (parentSessionId) {
					console.error(`Parent session: ${parentSessionId}`);
				}
				if (effectivePath && effectivePath !== path) {
					console.error(`Using inherited path: ${effectivePath}`);
				}
			}

			// Execute delegation - let errors propagate naturally
			const result = await delegate({
				task,
				timeout,
				debug,
				currentIteration: currentIteration || 0,
				maxIterations: maxIterations || 30,
				parentSessionId,
				path: effectivePath,
				provider,
				model,
				tracer
			});

			return result;
		}
	});
};