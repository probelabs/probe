/**
 * Common schemas and definitions for AI tools
 * @module tools/common
 */

import { z } from 'zod';

// Common schemas for tool parameters (used for internal execution after XML parsing)
export const searchSchema = z.object({
	query: z.string().describe('Search query with Elasticsearch syntax. Use quotes for exact matches, AND/OR for boolean logic, - for negation.'),
	path: z.string().optional().default('.').describe('Path to search in. For dependencies use "go:github.com/owner/repo", "js:package_name", or "rust:cargo_name" etc.')
});

export const querySchema = z.object({
	pattern: z.string().describe('AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, etc.'),
	path: z.string().optional().default('.').describe('Path to search in'),
	language: z.string().optional().default('rust').describe('Programming language to use for parsing'),
	allow_tests: z.boolean().optional().default(true).describe('Allow test files in search results')
});

export const extractSchema = z.object({
	targets: z.string().optional().describe('File paths or symbols to extract from. Formats: "file.js" (whole file), "file.js:42" (line 42), "file.js:10-20" (lines 10-20), "file.js#funcName" (symbol). Multiple targets separated by spaces.'),
	input_content: z.string().optional().describe('Text content to extract file paths from (alternative to targets)'),
	allow_tests: z.boolean().optional().default(true).describe('Include test files in extraction results')
});

export const delegateSchema = z.object({
	task: z.string().describe('The task to delegate to a subagent. Be specific about what needs to be accomplished.')
});

export const bashSchema = z.object({
	command: z.string().describe('The bash command to execute'),
	workingDirectory: z.string().optional().describe('Directory to execute the command in (optional)'),
	timeout: z.number().optional().describe('Command timeout in milliseconds (optional)'),
	env: z.record(z.string()).optional().describe('Additional environment variables (optional)')
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

**Session Management & Caching:**
- Ensure not to re-read the same symbols twice - reuse context from previous tool calls
- Probe returns a session ID on first run - reuse it for subsequent calls to avoid redundant searches
- Once data is returned, it's cached and won't return on next runs (this is expected behavior)

Parameters:
- query: (required) Search query with Elasticsearch syntax. Use quotes for exact matches ("functionName"), AND/OR for boolean logic, - for negation, + for important terms.
- path: (optional, default: '.') Path to search in. All dependencies located in /dep folder, under language sub folders, like this: "/dep/go/github.com/owner/repo", "/dep/js/package_name", or "/dep/rust/cargo_name" etc.

**Workflow:** Always start with search, then use extract for detailed context when needed.

Usage Example:

<examples>

User: Where is the login logic?
Assistant workflow:
1. <search>
<query>login AND auth AND token</query>
<path>.</path>
</search>
2. Now lets look closer: <extract>
<targets>session.rs#AuthService.login auth.rs:2-100</targets>
</extract>

User: How to calculate the total amount in the payments module?
<search>
<query>calculate AND payment</query>
<path>src/utils</path>
</search>

User: How do the user authentication and authorization work?
<search>
<query>+user AND (authentication OR authorization OR authz)</query>
<path>.</path>
</search>

User: Find all react imports in the project.
<search>
<query>"import" AND "react"</query>
<path>.</path>
</search>

User: Find how decompound library works?
<search>
<query>decompound</query>
<path>/dep/rust/decompound</path>
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
- allow_tests: (optional, default: true) Allow test files in search results (true/false).
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

**Multiple Extraction:** You can extract multiple symbols/files in one call by providing multiple file paths separated by spaces.

**Session Awareness:** Reuse context from previous tool calls. Don't re-extract the same symbols you already have.

Parameters:
- targets: (required) File paths or symbols to extract from. Formats: "file.js" (whole file), "file.js:42" (code block at line 42), "file.js:10-20" (lines 10-20), "file.js#funcName" (specific symbol). Multiple targets separated by spaces.
- input_content: (optional) Text content to extract file paths from (alternative to targets for processing diffs/logs).
- allow_tests: (optional, default: true) Include test files in extraction results.

Usage Example:

<examples>

User: Where is the login logic? (After search found relevant files)
<extract>
<targets>session.rs#AuthService.login auth.rs:2-100 config.rs#DatabaseConfig</targets>
</extract>

User: How does error handling work? (After search identified files)
<extract>
<targets>error.rs#ErrorType utils.rs#handle_error src/main.rs:50-80</targets>
</extract>

User: How RankManager works
<extract>
<targets>src/search/ranking.rs#RankManager</targets>
</extract>

User: Lets read the whole file
<extract>
<targets>src/search/ranking.rs</targets>
</extract>

User: Read the first 10 lines of the file
<extract>
<targets>src/search/ranking.rs:1-10</targets>
</extract>

User: Read file inside the dependency
<extract>
<targets>/dep/go/github.com/gorilla/mux/router.go</targets>
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
- No validation required - provide your complete answer directly inside the XML tags.
Usage Example:
<attempt_completion>
I have refactored the search module according to the requirements and verified the tests pass. The module now uses the new BM25 ranking algorithm and has improved error handling.
</attempt_completion>
`;

export const bashToolDefinition = `
## bash
Description: Execute bash commands for system exploration and development tasks. This tool has built-in security with allow/deny lists. By default, only safe read-only commands are allowed for code exploration.

Parameters:
- command: (required) The bash command to execute
- workingDirectory: (optional) Directory to execute the command in
- timeout: (optional) Command timeout in milliseconds
- env: (optional) Additional environment variables as an object

Security: Commands are filtered through allow/deny lists for safety:
- Allowed by default: ls, cat, git status, npm list, find, grep, etc.
- Denied by default: rm -rf, sudo, npm install, dangerous system commands

Usage Examples:

<examples>

User: What files are in the src directory?
<bash>
<command>ls -la src/</command>
</bash>

User: Show me the git status
<bash>
<command>git status</command>
</bash>

User: Find all TypeScript files
<bash>
<command>find . -name "*.ts" -type f</command>
</bash>

User: Check installed npm packages
<bash>
<command>npm list --depth=0</command>
</bash>

User: Search for TODO comments in code
<bash>
<command>grep -r "TODO" src/</command>
</bash>

User: Show recent git commits
<bash>
<command>git log --oneline -10</command>
</bash>

User: Check system info
<bash>
<command>uname -a</command>
</bash>

</examples>
`;

export const searchDescription = 'Search code in the repository using Elasticsearch-like query syntax. Use this tool first for any code-related questions.';
export const queryDescription = 'Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.';
export const extractDescription = 'Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files.';
export const delegateDescription = 'Automatically delegate big distinct tasks to specialized probe subagents within the agentic loop. Used by AI agents to break down complex requests into focused, parallel tasks.';
export const bashDescription = 'Execute bash commands for system exploration and development tasks. Secure by default with built-in allow/deny lists.';

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

/**
 * Get valid parameter names for a specific tool from its schema
 * @param {string} toolName - Name of the tool
 * @returns {string[]} - Array of valid parameter names for this tool
 */
function getValidParamsForTool(toolName) {
	// Map tool names to their schemas
	const schemaMap = {
		search: searchSchema,
		query: querySchema,
		extract: extractSchema,
		delegate: delegateSchema,
		bash: bashSchema,
		attempt_completion: attemptCompletionSchema
	};

	const schema = schemaMap[toolName];
	if (!schema) {
		// For tools without schema (listFiles, searchFiles, implement), return common params
		// These are the shared params that appear across multiple tools
		return ['path', 'directory', 'pattern', 'recursive', 'includeHidden', 'task', 'files', 'autoCommits', 'result'];
	}

	// For attempt_completion, it has custom validation, just return 'result'
	if (toolName === 'attempt_completion') {
		return ['result'];
	}

	// Extract keys from Zod schema
	if (schema && schema._def && schema._def.shape) {
		return Object.keys(schema._def.shape());
	}

	// Fallback: return empty array if we can't extract schema keys
	return [];
}

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

		// For attempt_completion, use lastIndexOf to find the LAST occurrence of closing tag
		// This prevents issues where the content contains the closing tag string (e.g., in regex patterns)
		// For other tools, use indexOf from the opening tag position
		let closeIndex;
		if (toolName === 'attempt_completion') {
			// Find the last occurrence of the closing tag in the entire string
			// This assumes attempt_completion doesn't have nested tags of the same name
			closeIndex = xmlString.lastIndexOf(closeTag);
			// Make sure the closing tag is after the opening tag
			if (closeIndex !== -1 && closeIndex <= openIndex + openTag.length) {
				closeIndex = -1; // Invalid, treat as no closing tag
			}
		} else {
			closeIndex = xmlString.indexOf(closeTag, openIndex + openTag.length);
		}

		let hasClosingTag = closeIndex !== -1;

		// If no closing tag found, use content until end of string
		// This makes the parser more resilient to AI formatting errors
		if (closeIndex === -1) {
			closeIndex = xmlString.length;
		}

		// Extract the content between tags (or until end if no closing tag)
		const innerContent = xmlString.substring(
			openIndex + openTag.length,
			closeIndex
		);

		const params = {};

		// Get valid parameters for this specific tool from its schema
		const validParams = getValidParamsForTool(toolName);

		// Parse parameters using string-based approach for better safety
		// Only look for parameters that are valid for this specific tool
		for (const paramName of validParams) {
			const paramOpenTag = `<${paramName}>`;
			const paramCloseTag = `</${paramName}>`;

			const paramOpenIndex = innerContent.indexOf(paramOpenTag);
			if (paramOpenIndex === -1) {
				continue; // Parameter not found
			}

			let paramCloseIndex = innerContent.indexOf(paramCloseTag, paramOpenIndex + paramOpenTag.length);

			// Handle unclosed parameter tags - use content until next tag or end of content
			if (paramCloseIndex === -1) {
				// Find the next opening tag after this parameter
				let nextTagIndex = innerContent.length;
				for (const nextParam of validParams) {
					const nextOpenTag = `<${nextParam}>`;
					const nextIndex = innerContent.indexOf(nextOpenTag, paramOpenIndex + paramOpenTag.length);
					if (nextIndex !== -1 && nextIndex < nextTagIndex) {
						nextTagIndex = nextIndex;
					}
				}
				paramCloseIndex = nextTagIndex;
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

		// Special handling for attempt_completion - use entire inner content as result
		if (toolName === 'attempt_completion') {
			params['result'] = innerContent.trim();
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

/**
 * Parse targets string into array of file specifications
 * Handles both space-separated and comma-separated targets for extract tool
 *
 * @param {string} targets - Space or comma-separated file targets (e.g., "file1.rs:10-20, file2.rs#symbol")
 * @returns {string[]} Array of individual file specifications
 *
 * @example
 * parseTargets("file1.rs:10-20 file2.rs:30-40")
 * // Returns: ["file1.rs:10-20", "file2.rs:30-40"]
 *
 * @example
 * parseTargets("file1.rs:10-20, file2.rs:30-40")
 * // Returns: ["file1.rs:10-20", "file2.rs:30-40"]
 *
 * @example
 * parseTargets("session.rs#AuthService.login auth.rs:2-100 config.rs#DatabaseConfig")
 * // Returns: ["session.rs#AuthService.login", "auth.rs:2-100", "config.rs#DatabaseConfig"]
 */
export function parseTargets(targets) {
	if (!targets || typeof targets !== 'string') {
		return [];
	}

	// Split on any whitespace or comma (with optional surrounding whitespace) and filter out empty strings
	return targets.split(/[\s,]+/).filter(f => f.length > 0);
}