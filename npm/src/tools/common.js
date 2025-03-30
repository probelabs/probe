/**
 * Common schemas and definitions for AI tools
 * @module tools/common
 */

import { z } from 'zod';

// Common schemas for tool parameters (used for internal execution after XML parsing)
export const searchSchema = z.object({
	query: z.string().describe('Search query with Elasticsearch syntax. Use + for important terms.'),
	path: z.string().optional().default('.').describe('Path to search in. For dependencies use "go:github.com/owner/repo", "js:package_name", or "rust:cargo_name" etc.'),
	allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results'),
	exact: z.boolean().optional().default(false).describe('Perform exact search without tokenization (case-insensitive)'),
	maxResults: z.number().optional().describe('Maximum number of results to return'),
	maxTokens: z.number().optional().default(10000).describe('Maximum number of tokens to return'),
	language: z.string().optional().describe('Limit search to files of a specific programming language')
});

export const querySchema = z.object({
	pattern: z.string().describe('AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, etc.'),
	path: z.string().optional().default('.').describe('Path to search in'),
	language: z.string().optional().default('rust').describe('Programming language to use for parsing'),
	allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results')
});

export const extractSchema = z.object({
	file_path: z.string().optional().describe('Path to the file to extract from. Can include line numbers or symbol names'),
	input_content: z.string().optional().describe('Text content to extract file paths from'),
	line: z.number().optional().describe('Start line number to extract a specific code block'),
	end_line: z.number().optional().describe('End line number for extracting a range of lines'),
	allow_tests: z.boolean().optional().default(false).describe('Allow test files and test code blocks'),
	context_lines: z.number().optional().default(10).describe('Number of context lines to include'),
	format: z.string().optional().default('plain').describe('Output format (plain, markdown, json, color)')
});

// Schema for the new attempt_completion tool
export const attemptCompletionSchema = z.object({
	result: z.string().describe('The final result of the task. Formulate this result in a way that is final and does not require further input from the user. Do not end your result with questions or offers for further assistance.'),
	command: z.string().optional().describe('A CLI command to execute to show a live demo of the result to the user (e.g., `open index.html`). Do not use commands like `echo` or `cat` that merely print text.')
});


// Tool descriptions for the system prompt (using XML format)

export const searchToolDefinition = `
## search
Description: Search code in the repository using Elasticsearch-like query syntax. Use this tool first for any code-related questions.
You need to focus on main keywords when constructing the query, and use elastic search syntax like OR AND and brackets to group keywords.
Parameters:
- query: (required) Search query with Elasticsearch syntax. You can use + for important terms, and - for negation.
- path: (optional, default: '.') Path to search in. All dependencies located in /dep folder, under language sub folders, like this: "/dep/go/github.com/owner/repo", "/dep/js/package_name", or "/dep/rust/cargo_name" etc.
- allow_tests: (optional, default: false) Allow test files in search results (true/false).
- exact: (optional, default: false) Perform exact pricise search. Use it when you already know function or struct name, or some other code block, and want exact match.
- maxResults: (optional) Maximum number of results to return (number).
- maxTokens: (optional, default: 10000) Maximum number of tokens to return (number).
- language: (optional) Limit search to files of a specific programming language (e.g., 'rust', 'javascript', 'python', etc.).
Usage Example:
<search>
<query>+important +calculate</query>
<path>src/utils</path>
<allow_tests>false</allow_tests>
<exact>true</exact>
<language>javascript</language>
</search>
`;

export const queryToolDefinition = `
## query
Description: Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.
Parameters:
- pattern: (required) AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, etc.
- path: (optional, default: '.') Path to search in.
- language: (optional, default: 'rust') Programming language to use for parsing.
- allow_tests: (optional, default: false) Allow test files in search results (true/false).
Usage Example:
<query>
<pattern>function $FUNC($$$PARAMS) { $$$BODY }</pattern>
<path>src/parser</path>
<language>javascript</language>
</query>
`;

export const extractToolDefinition = `
## extract
Description: Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files. It can be used to read full files as well.
Parameters:
- file_path: (required) Path to the file to extract from. Can include line numbers or symbol names (e.g., 'src/main.rs:10-20', 'src/utils.js#myFunction').
- line: (optional) Start line number to extract a specific code block. Use with end_line for ranges.
- end_line: (optional) End line number for extracting a range of lines.
- allow_tests: (optional, default: false) Allow test files and test code blocks (true/false).
Usage Example:
<extract>
<file_path>src/search/ranking.rs#RankManager</file_path>
</extract>
`;

export const attemptCompletionToolDefinition = `
## attempt_completion
Description: Use this tool ONLY when the task is fully complete and you have received confirmation of success for all previous tool uses. Presents the final result to the user.
Parameters:
- result: (required) The final result of the task. Formulate this result concisely and definitively. Do not end with questions or offers for further assistance.
- command: (optional) A CLI command to demonstrate the result (e.g., 'open index.html'). Avoid simple print commands like 'echo'.
Usage Example:
<attempt_completion>
<result>I have refactored the search module according to the requirements and verified the tests pass.</result>
<command>cargo test --lib</command>
</attempt_completion>
`;

export const searchDescription = 'Search code in the repository using Elasticsearch-like query syntax. Use this tool first for any code-related questions.';
export const queryDescription = 'Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.';
export const extractDescription = 'Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files.';

// Simple XML parser helper
export function parseXmlToolCall(xmlString) {
	const toolMatch = xmlString.match(/<([a-zA-Z0-9_]+)>([\s\S]*?)<\/\1>/);
	if (!toolMatch) {
		return null;
	}

	const toolName = toolMatch[1];
	const innerContent = toolMatch[2];
	const params = {};

	const paramRegex = /<([a-zA-Z0-9_]+)>([\s\S]*?)<\/\1>/g;
	let paramMatch;
	while ((paramMatch = paramRegex.exec(innerContent)) !== null) {
		const paramName = paramMatch[1];
		let paramValue = paramMatch[2].trim();

		// Basic type inference (can be improved)
		if (paramValue.toLowerCase() === 'true') {
			paramValue = true;
		} else if (paramValue.toLowerCase() === 'false') {
			paramValue = false;
		} else if (!isNaN(paramValue) && paramValue.trim() !== '') {
			// Check if it's potentially a number (handle integers and floats)
			const num = Number(paramValue);
			if (Number.isFinite(num)) { // Use Number.isFinite to avoid Infinity/NaN
				paramValue = num;
			}
			// Keep as string if not a valid finite number
		}

		params[paramName] = paramValue;
	}

	// Special handling for attempt_completion where result might contain nested XML/code
	if (toolName === 'attempt_completion') {
		const resultMatch = innerContent.match(/<result>([\s\S]*?)<\/result>/);
		if (resultMatch) {
			params['result'] = resultMatch[1].trim(); // Keep result content as is
		}
		const commandMatch = innerContent.match(/<command>([\s\S]*?)<\/command>/);
		if (commandMatch) {
			params['command'] = commandMatch[1].trim();
		}
	}


	return { toolName, params };
}