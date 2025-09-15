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

export const delegateSchema = z.object({
	task: z.string().describe('The task to delegate to a subagent. Be specific about what needs to be accomplished.')
});

// Schema for the attempt_completion tool - flexible validation for direct XML response
export const attemptCompletionSchema = {
	// Custom validation that requires result parameter but allows direct XML response
	safeParse: (params) => {
		// Validate that params is an object
		if (!params || typeof params !== 'object') {
			return {
				success: false,
				error: {
					issues: [{
						code: 'invalid_type',
						expected: 'object',
						received: typeof params,
						path: [],
						message: 'Expected object'
					}]
				}
			};
		}

		// Validate that result parameter exists and is a string
		if (!('result' in params)) {
			return {
				success: false,
				error: {
					issues: [{
						code: 'invalid_type',
						expected: 'string',
						received: 'undefined',
						path: ['result'],
						message: 'Required'
					}]
				}
			};
		}

		if (typeof params.result !== 'string') {
			return {
				success: false,
				error: {
					issues: [{
						code: 'invalid_type',
						expected: 'string',
						received: typeof params.result,
						path: ['result'],
						message: 'Expected string'
					}]
				}
			};
		}

		// Filter out command parameter if present (legacy compatibility)
		const filteredData = { result: params.result };
		
		return {
			success: true,
			data: filteredData
		};
	}
};


// Tool descriptions for the system prompt (using XML format)

export const searchToolDefinition = `
## search
Description: Search code in the repository using Elasticsearch query syntax (except field based queries, e.g. "filename:..." NOT supported).

You need to focus on main keywords when constructing the query, and always use elastic search syntax like OR AND and brackets to group keywords.
Parameters:
- query: (required) Search query with Elasticsearch syntax. You can use + for important terms, and - for negation.
- path: (required) Path to search in. All dependencies located in /dep folder, under language sub folders, like this: "/dep/go/github.com/owner/repo", "/dep/js/package_name", or "/dep/rust/cargo_name" etc. YOU SHOULD ALWAYS provide FULL PATH when searching dependencies, including depency name.
- allow_tests: (optional, default: false) Allow test files in search results (true/false).
- exact: (optional, default: false) Perform exact pricise search. Use it when you already know function or struct name, or some other code block, and want exact match.
- maxResults: (optional) Maximum number of results to return (number).
- maxTokens: (optional, default: 10000) Maximum number of tokens to return (number).
- language: (optional) Limit search to files of a specific programming language (e.g., 'rust', 'js', 'python', 'go' etc.).


Usage Example:

<examples>

User: How to calculate the total amount in the payments module?
<search>
<query>calculate AND payment</query>
<path>src/utils</path>
<allow_tests>false</allow_tests>
</search>

User: How do the user authentication and authorization work?
<search>
<query>+user and (authentification OR authroization OR authz)</query>
<path>.</path>
<allow_tests>true</allow_tests>
<language>go</language>
</search>

User: Find all react imports in the project.
<search>
<query>import { react }</query>
<path>.</path>
<exact>true</exact>
<language>js</language>
</search>


User: Find how decompoud library works?
<search>
<query>import { react }</query>
<path>/dep/rust/decompound</path>
<language>rust</language>
</search>

</examples>
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

<examples>

<query>
<pattern>function $FUNC($$$PARAMS) { $$$BODY }</pattern>
<path>src/parser</path>
<language>js</language>
</query>

</examples>
`;

export const extractToolDefinition = `
## extract
Description: Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files. It can be used to read full files as well. 
Full file extraction should be the LAST RESORT! Always prefer search.

Parameters:
- file_path: (required) Path to the file to extract from. Can include line numbers or symbol names (e.g., 'src/main.rs:10-20', 'src/utils.js#myFunction').
- line: (optional) Start line number to extract a specific code block. Use with end_line for ranges.
- end_line: (optional) End line number for extracting a range of lines.
- allow_tests: (optional, default: false) Allow test files and test code blocks (true/false).
Usage Example:

<examples>

User: How RankManager works
<extract>
<file_path>src/search/ranking.rs#RankManager</file_path>
</extract>

User: Lets read the whole file
<extract>
<file_path>src/search/ranking.rs</file_path>
</extract>

User: Read the first 10 lines of the file
<extract>
<file_path>src/search/ranking.rs</file_path>
<line>1</line>
<end_line>10</end_line>
</extract>

User: Read file inside the dependency
<extract>
<file_path>/dep/go/github.com/gorilla/mux/router.go</file_path>
</extract>


</examples>
`;

export const delegateToolDefinition = `
## delegate
Description: Automatically delegate big distinct tasks to specialized probe subagents within the agentic loop. Use this when you recognize that a user's request involves multiple large, distinct components that would benefit from parallel processing or specialized focus. The AI agent should automatically identify opportunities for task separation and use delegation without explicit user instruction.

Parameters:
- task: (required) A complete, self-contained task that can be executed independently by a subagent. Should be specific and focused on one area of expertise.

Usage Pattern:
When the AI agent encounters complex multi-part requests, it should automatically break them down and delegate:

<delegate>
<task>Analyze all authentication and authorization code in the codebase for security vulnerabilities and provide specific remediation recommendations</task>
</delegate>

<delegate>
<task>Review database queries and API endpoints for performance bottlenecks and suggest optimization strategies</task>
</delegate>

The agent uses this tool automatically when it identifies that work can be separated into distinct, parallel tasks for more efficient processing.
`;

export const attemptCompletionToolDefinition = `
## attempt_completion
Description: Use this tool ONLY when the task is fully complete and you have received confirmation of success for all previous tool uses. Presents the final result to the user. You can provide your response directly inside the XML tags without any parameter wrapper.
Parameters:
- No validation required - provide your complete answer directly inside the XML tags or use the <result> parameter (both formats supported).
Usage Examples:
<attempt_completion>
<result>I have refactored the search module according to the requirements and verified the tests pass. The module now uses the new BM25 ranking algorithm and has improved error handling.</result>
</attempt_completion>

Or direct response:
<attempt_completion>
I have refactored the search module according to the requirements and verified the tests pass. The module now uses the new BM25 ranking algorithm and has improved error handling.
</attempt_completion>
`;

export const searchDescription = 'Search code in the repository using Elasticsearch-like query syntax. Use this tool first for any code-related questions.';
export const queryDescription = 'Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.';
export const extractDescription = 'Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files.';
export const delegateDescription = 'Automatically delegate big distinct tasks to specialized probe subagents within the agentic loop. Used by AI agents to break down complex requests into focused, parallel tasks.';

// Valid tool names that should be parsed as tool calls
const DEFAULT_VALID_TOOLS = [
	'search',
	'query', 
	'extract',
	'delegate',
	'listFiles',
	'searchFiles',
	'implement',
	'attempt_completion'
];

// Simple XML parser helper - safer string-based approach
export function parseXmlToolCall(xmlString, validTools = DEFAULT_VALID_TOOLS) {
	// Look for each valid tool name specifically using string search
	for (const toolName of validTools) {
		const openTag = `<${toolName}>`;
		const closeTag = `</${toolName}>`;
		
		const openIndex = xmlString.indexOf(openTag);
		if (openIndex === -1) {
			continue; // Tool not found, try next tool
		}
		
		const closeIndex = xmlString.indexOf(closeTag, openIndex + openTag.length);
		if (closeIndex === -1) {
			continue; // No closing tag found, try next tool
		}
		
		// Extract the content between tags
		const innerContent = xmlString.substring(
			openIndex + openTag.length, 
			closeIndex
		);
		
		const params = {};

		// Parse parameters using string-based approach for better safety
		// Common parameter names to look for (can be extended as needed)
		// Note: includes both camelCase and underscore_case variants to handle inconsistencies
		const commonParams = ['query', 'file_path', 'line', 'end_line', 'path', 'recursive', 'includeHidden', 
		                      'max_results', 'maxResults', 'result', 'command', 'description', 'task', 'param', 'pattern',
		                      'allow_tests', 'exact', 'maxTokens', 'language', 'input_content',
		                      'context_lines', 'format', 'directory', 'autoCommits', 'files'];
		
		for (const paramName of commonParams) {
			const paramOpenTag = `<${paramName}>`;
			const paramCloseTag = `</${paramName}>`;
			
			const paramOpenIndex = innerContent.indexOf(paramOpenTag);
			if (paramOpenIndex === -1) {
				continue; // Parameter not found
			}
			
			const paramCloseIndex = innerContent.indexOf(paramCloseTag, paramOpenIndex + paramOpenTag.length);
			if (paramCloseIndex === -1) {
				continue; // No closing tag found
			}
			
			let paramValue = innerContent.substring(
				paramOpenIndex + paramOpenTag.length,
				paramCloseIndex
			).trim();

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

		// Special handling for attempt_completion - allow direct XML response without validation
		if (toolName === 'attempt_completion') {
			// First try to find <result> tags (backward compatibility) using string-based approach
			const resultOpenTag = '<result>';
			const resultCloseTag = '</result>';
			const resultOpenIndex = innerContent.indexOf(resultOpenTag);
			
			if (resultOpenIndex !== -1) {
				const resultCloseIndex = innerContent.indexOf(resultCloseTag, resultOpenIndex + resultOpenTag.length);
				if (resultCloseIndex !== -1) {
					params['result'] = innerContent.substring(
						resultOpenIndex + resultOpenTag.length,
						resultCloseIndex
					).trim();
				}
			} else {
				// Count how many parameters were parsed (excluding command which will be removed)
				const paramsCount = Object.keys(params).filter(key => key !== 'command').length;
				
				// If no <result> tags and no other meaningful parameters parsed, use the entire inner content as direct XML response
				if (paramsCount === 0) {
					params['result'] = innerContent.trim();
				}
			}
			// Remove command parameter if it was parsed by generic logic above (legacy compatibility)
			if (params.command) {
				delete params.command;
			}
		}

		// Return the first valid tool found
		return { toolName, params };
	}

	// No valid tool found
	return null;
}

/**
 * Creates an improved preview of a message showing start and end portions
 * @param {string} message - The message to preview
 * @param {number} charsPerSide - Number of characters to show from start and end (default: 200)
 * @returns {string} Formatted preview string
 */
export function createMessagePreview(message, charsPerSide = 200) {
	if (message === null || message === undefined) {
		return 'null/undefined';
	}
	
	if (typeof message !== 'string') {
		return 'null/undefined';
	}
	
	const totalChars = charsPerSide * 2;
	
	if (message.length <= totalChars) {
		// Message is short enough to show completely
		return message;
	}
	
	// Message is longer - show start and end with ... in between
	const start = message.substring(0, charsPerSide);
	const end = message.substring(message.length - charsPerSide);
	
	return `${start}...${end}`;
}