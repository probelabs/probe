/**
 * Tools for Vercel AI SDK
 * @module tools/vercel
 */

import { tool } from 'ai';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { searchSchema, querySchema, extractSchema, searchDescription, queryDescription, extractDescription } from './common.js';

// Tool for searching code using the Probe CLI
export const searchTool = tool({
	name: 'search',
	description: searchDescription,
	parameters: searchSchema,
	execute: async ({ query: searchQuery, path, exact, allow_tests, maxResults, maxTokens = 40000 }) => {
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
});

// Tool for querying code using ast-grep patterns
export const queryTool = tool({
	name: 'query',
	description: queryDescription,
	parameters: querySchema,
	execute: async ({ pattern, path, language, allow_tests }) => {
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
});

// Tool for extracting code blocks from files
export const extractTool = tool({
	name: 'extract',
	description: extractDescription,
	parameters: extractSchema,
	execute: async ({ file_path, line, end_line, allow_tests, context_lines, format }) => {
		try {
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
			return `Error: ${error.message}`;
		}
	}
});