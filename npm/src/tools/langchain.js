/**
 * Tools for LangChain
 * @module tools/langchain
 */

import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { searchSchema, querySchema, extractSchema, searchDescription, queryDescription, extractDescription, parseTargets } from './common.js';

// LangChain tool for searching code
export function createSearchTool(options = {}) {
	const { cwd } = options;

	return {
		name: 'search',
		description: searchDescription,
		schema: searchSchema,
		func: async ({ query: searchQuery, path, allow_tests, exact, maxResults, maxTokens = 20000, language, session, nextPage }) => {
			try {
				const results = await search({
					query: searchQuery,
					path,
					cwd, // Working directory for resolving relative paths
					allowTests: allow_tests ?? true,
					exact,
					json: false,
					maxResults,
					maxTokens,
					language,
					session,
					nextPage
				});

				return results;
			} catch (error) {
				console.error('Error executing search command:', error);
				return `Error executing search command: ${error.message}`;
			}
		}
	};
}

// LangChain tool for querying code
export function createQueryTool(options = {}) {
	const { cwd } = options;

	return {
		name: 'query',
		description: queryDescription,
		schema: querySchema,
		func: async ({ pattern, path, language, allow_tests }) => {
			try {
				const results = await query({
					pattern,
					path,
					cwd, // Working directory for resolving relative paths
					language,
					allowTests: allow_tests ?? true,
					json: false
				});

				return results;
			} catch (error) {
				console.error('Error executing query command:', error);
				return `Error executing query command: ${error.message}`;
			}
		}
	};
}

// LangChain tool for extracting code
export function createExtractTool(options = {}) {
	const { cwd } = options;

	return {
		name: 'extract',
		description: extractDescription,
		schema: extractSchema,
		func: async ({ targets, line, end_line, allow_tests, context_lines, format }) => {
			try {
				// Split targets on whitespace to support multiple targets in one call
				const files = parseTargets(targets);

				const results = await extract({
					files,
					cwd, // Working directory for resolving relative paths
					allowTests: allow_tests ?? true,
					contextLines: context_lines,
					format
				});

				return results;
			} catch (error) {
				console.error('Error executing extract command:', error);
				return `Error executing extract command: ${error.message}`;
			}
		}
	};
}