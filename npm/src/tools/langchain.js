/**
 * Tools for LangChain
 * @module tools/langchain
 */

import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { searchSchema, querySchema, extractSchema, searchDescription, queryDescription, extractDescription } from './common.js';

// LangChain tool for searching code
export function createSearchTool() {
	return {
		name: 'search',
		description: searchDescription,
		schema: searchSchema,
		func: async ({ query: searchQuery, path, exact, allow_tests, maxResults, maxTokens = 40000 }) => {
			try {
				const results = await search({
					query: searchQuery,
					path,
					exact,
					allow_tests,
					json: false,
					maxResults,
					maxTokens
				});

				return results;
			} catch (error) {
				return `Error: ${error.message}`;
			}
		}
	};
}

// LangChain tool for querying code
export function createQueryTool() {
	return {
		name: 'query',
		description: queryDescription,
		schema: querySchema,
		func: async ({ pattern, path, language, allow_tests }) => {
			try {
				const results = await query({
					pattern,
					path,
					language,
					allow_tests,
					json: false
				});

				return results;
			} catch (error) {
				return `Error: ${error.message}`;
			}
		}
	};
}

// LangChain tool for extracting code
export function createExtractTool() {
	return {
		name: 'extract',
		description: extractDescription,
		schema: extractSchema,
		func: async ({ file_path, line, end_line, allow_tests, context_lines, format }) => {
			try {
				const files = [file_path];

				const results = await extract({
					files,
					allowTests: allow_tests,
					contextLines: context_lines,
					format
				});

				return results;
			} catch (error) {
				return `Error: ${error.message}`;
			}
		}
	};
}