/**
 * Common schemas and definitions for AI tools
 * @module tools/common
 */

import { z } from 'zod';
import { resolve, isAbsolute } from 'path';
import { editSchema, createSchema } from './edit.js';
import { taskSchema } from '../agent/tasks/taskTool.js';

// Common schemas for tool parameters (used for internal execution after XML parsing)
export const searchSchema = z.object({
	query: z.string().describe('Search query with Elasticsearch syntax. Use quotes for exact matches, AND/OR for boolean logic, - for negation.'),
	path: z.string().optional().default('.').describe('Path to search in. For dependencies use "go:github.com/owner/repo", "js:package_name", or "rust:cargo_name" etc.'),
	exact: z.boolean().optional().default(false).describe('Default (false) enables stemming and keyword splitting for exploratory search - "getUserData" matches "get", "user", "data", etc. Set true for precise symbol lookup where "getUserData" matches only "getUserData". Use true when you know the exact symbol name.'),
	session: z.string().optional().describe('Session ID for result caching and pagination. Pass the session ID from a previous search to get additional results (next page). Results already shown in a session are automatically excluded. Omit for a fresh search.'),
	nextPage: z.boolean().optional().default(false).describe('Set to true when requesting the next page of results. Requires passing the same session ID from the previous search output.')
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

export const listSkillsSchema = z.object({
	filter: z.string().optional().describe('Optional substring filter to match skill names or descriptions.')
});

export const useSkillSchema = z.object({
	name: z.string().describe('Skill name to load and activate.')
});

export const bashSchema = z.object({
	command: z.string().describe('The bash command to execute'),
	workingDirectory: z.string().optional().describe('Directory to execute the command in (optional)'),
	timeout: z.number().optional().describe('Command timeout in milliseconds (optional)'),
	env: z.record(z.string()).optional().describe('Additional environment variables (optional)')
});

export const analyzeAllSchema = z.object({
	question: z.string().min(1).describe('Free-form question to answer (e.g., "What features are customers using?", "List all API endpoints"). The AI will automatically explore the repository, test search strategies, and synthesize a comprehensive answer.'),
	path: z.string().optional().default('.').describe('Directory path to search in')
});

export const executePlanSchema = z.object({
	code: z.string().min(1).describe('JavaScript DSL code to execute. All function calls look synchronous — do NOT use async/await. Use map(items, fn) for batch operations. Use LLM(instruction, data) for AI processing.'),
	description: z.string().optional().describe('Human-readable description of what this plan does, for logging.')
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
Description: Search code in the repository. You may provide a free-form question about the code or a concise Elasticsearch-style keyword query (field based queries, e.g. "filename:..." NOT supported).
Note: This tool may internally use a dedicated search subagent when search delegation is enabled. This is separate from the "delegate" tool and does not require an explicit delegate call.

You need to focus on main keywords when constructing the query, and always use elastic search syntax like OR AND and brackets to group keywords.

**Session Management & Caching:**
- Ensure not to re-read the same symbols twice - reuse context from previous tool calls
- Probe returns a session ID on first run - reuse it for subsequent calls to avoid redundant searches
- Once data is returned, it's cached and won't return on next runs (this is expected behavior)

Parameters:
- query: (required) Search query. Free-form questions are accepted, but for best results prefer Elasticsearch-style syntax with quotes for exact matches ("functionName"), AND/OR for boolean logic, - for negation, + for important terms.
- path: (optional, default: '.') Path to search in. All dependencies located in /dep folder, under language sub folders, like this: "/dep/go/github.com/owner/repo", "/dep/js/package_name", or "/dep/rust/cargo_name" etc.
- exact: (optional, default: false) Set to true for precise symbol lookup without stemming/tokenization. Use when you know the exact symbol name (e.g., "getUserData" matches only "getUserData", not "get", "user", "data").
- session: (optional) Session ID for pagination. Pass the session ID returned from a previous search to get the next page of results. Results already shown are automatically excluded.
- nextPage: (optional, default: false) Set to true when requesting the next page of results. Requires passing the same session ID from the previous search.

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

export const analyzeAllToolDefinition = `
## analyze_all
Description: Intelligent bulk data analysis tool. Process ALL data matching your question using a 3-phase approach:
1. **PLANNING**: AI analyzes your question and determines the optimal search strategy
2. **PROCESSING**: Map-reduce processes all matching data in parallel chunks
3. **SYNTHESIS**: Comprehensive answer with evidence and organization

**Use this for questions requiring 100% data coverage:**
- "What features are customers using?"
- "List all API endpoints in the codebase"
- "Summarize the error handling patterns"
- "Count all TODO comments and their contexts"

**Do NOT use for:**
- Simple searches where a sample is sufficient
- Finding a specific function or class
- Quick exploration (use search instead)

**WARNING:** Makes multiple LLM calls - slower and costlier than search.

Parameters:
- question: (required) Free-form question to answer - the AI determines the best search strategy automatically
- path: (optional) Directory to search in (default: current directory)

<examples>

User: What are all the different tools available in this codebase?
<analyze_all>
<question>What are all the different tools available in this codebase and what do they do?</question>
<path>./src</path>
</analyze_all>

User: I need to understand all the error handling patterns
<analyze_all>
<question>What error handling patterns are used throughout the codebase? Include examples.</question>
</analyze_all>

User: Count and categorize all the environment variables
<analyze_all>
<question>What environment variables are used? Categorize them by purpose.</question>
<path>./src</path>
</analyze_all>

</examples>
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

export const googleSearchToolDefinition = `
## gemini_google_search (Gemini Built-in)
Description: Web search powered by Google. This is a built-in Gemini capability that automatically searches the web when the model needs current information. The model decides when to search and integrates results directly into its response with source citations.

This tool is invoked automatically by the model — you do NOT need to use XML tool calls for it. Simply ask questions that require up-to-date or real-world information and the model will search the web as needed.

Capabilities:
- Real-time web search with grounded citations
- Automatic query generation and result synthesis
- Source attribution with URLs
`;

export const urlContextToolDefinition = `
## gemini_url_context (Gemini Built-in)
Description: URL content reader powered by Google. This is a built-in Gemini capability that automatically fetches and analyzes the content of URLs mentioned in the conversation. When you include URLs in your message, the model can read and understand their content.

This tool is invoked automatically by the model — you do NOT need to use XML tool calls for it. Simply include URLs in your message and the model will fetch and analyze their content.

Capabilities:
- Fetch and read web page content from URLs in the prompt
- Supports up to 20 URLs per request
- Processes HTML content (does not execute JavaScript)
`;

export const searchDescription = 'Search code in the repository. Free-form questions are accepted, but Elasticsearch-style keyword queries work best. Use this tool first for any code-related questions.';
export const queryDescription = 'Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.';
export const extractDescription = 'Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files.';
export const delegateDescription = 'Automatically delegate big distinct tasks to specialized probe subagents within the agentic loop. Used by AI agents to break down complex requests into focused, parallel tasks.';
export const bashDescription = 'Execute bash commands for system exploration and development tasks. Secure by default with built-in allow/deny lists.';
export const analyzeAllDescription = 'Answer questions that require analyzing ALL matching data in the codebase. Use for aggregate questions like "What features exist?", "List all API endpoints", "Count TODO comments". The AI automatically plans the search strategy, processes all results via map-reduce, and synthesizes a comprehensive answer. WARNING: Slower than search - only use when you need complete coverage.';

// Valid tool names that should be parsed as tool calls
// This is the canonical list - all other tool lists should reference this
export const DEFAULT_VALID_TOOLS = [
	'search',
	'query',
	'extract',
	'delegate',
	'analyze_all',
	'execute_plan',
	'listSkills',
	'useSkill',
	'listFiles',
	'searchFiles',
	'implement',
	'bash',
	'task',
	'attempt_completion'
];

/**
 * Build a regex pattern to match any tool tag from the valid tools list
 * @param {string[]} tools - List of tool names (defaults to DEFAULT_VALID_TOOLS)
 * @returns {RegExp} - Regex pattern to match tool opening tags
 */
export function buildToolTagPattern(tools = DEFAULT_VALID_TOOLS) {
	// Also include attempt_complete as an alias for attempt_completion
	const allTools = [...tools];
	if (allTools.includes('attempt_completion') && !allTools.includes('attempt_complete')) {
		allTools.push('attempt_complete');
	}
	const escaped = allTools.map(t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
	return new RegExp(`<(${escaped.join('|')})>`);
}

/**
 * Get valid parameter names for a specific tool from its schema
 * @param {string} toolName - Name of the tool
 * @returns {string[]} - Array of valid parameter names for this tool
 */
function getValidParamsForTool(toolName) {
	// Map tool names to their schemas (supports both Zod and JSON Schema formats)
	const schemaMap = {
		search: searchSchema,
		query: querySchema,
		extract: extractSchema,
		delegate: delegateSchema,
		analyze_all: analyzeAllSchema,
		execute_plan: executePlanSchema,
		listSkills: listSkillsSchema,
		useSkill: useSkillSchema,
		bash: bashSchema,
		task: taskSchema,
		attempt_completion: attemptCompletionSchema,
		edit: editSchema,
		create: createSchema
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
	if (schema._def && schema._def.shape) {
		return Object.keys(schema._def.shape());
	}

	// Extract keys from JSON Schema (used by edit and create tools)
	if (schema.properties) {
		return Object.keys(schema.properties);
	}

	// Fallback: return empty array if we can't extract schema keys
	return [];
}

// Simple XML parser helper - safer string-based approach
export function parseXmlToolCall(xmlString, validTools = DEFAULT_VALID_TOOLS) {
	// Find the tool that appears EARLIEST in the string
	// This prevents parameter tags (like <query> inside <analyze_all>) from being matched as tools
	let earliestToolName = null;
	let earliestOpenIndex = Infinity;

	for (const toolName of validTools) {
		const openTag = `<${toolName}>`;
		const openIndex = xmlString.indexOf(openTag);
		if (openIndex !== -1 && openIndex < earliestOpenIndex) {
			earliestOpenIndex = openIndex;
			earliestToolName = toolName;
		}
	}

	// No valid tool found
	if (earliestToolName === null) {
		return null;
	}

	const toolName = earliestToolName;
	const openTag = `<${toolName}>`;
	const closeTag = `</${toolName}>`;
	const openIndex = earliestOpenIndex;

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

	// Return the parsed tool call
	return { toolName, params };
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
 * Detect if the response contains an XML-style tool tag that wasn't recognized
 * This helps identify when the AI tried to use a tool that's not in the validTools list
 *
 * @param {string} xmlString - The XML string to search
 * @param {string[]} validTools - List of valid tool names that would have been recognized
 * @returns {string|null} - The unrecognized tool name, or null if no unrecognized tools found
 */
export function detectUnrecognizedToolCall(xmlString, validTools) {
	if (!xmlString || typeof xmlString !== 'string') {
		return null;
	}

	// Common tool names that AI might try to use (these should appear as top-level tags)
	const knownToolNames = [
		'search', 'query', 'extract', 'listFiles', 'searchFiles',
		'listSkills', 'useSkill', 'readImage', 'implement', 'edit',
		'create', 'delegate', 'bash', 'task', 'attempt_completion',
		'attempt_complete', 'read_file', 'write_file', 'run_command',
		'grep', 'find', 'cat', 'list_directory'
	];

	// Look for XML tags that match known tool patterns
	// Only consider a tag as a tool call if:
	// 1. It's not in the validTools list (wouldn't have been parsed)
	// 2. It appears as a top-level tag (not nested inside another tool)
	for (const toolName of knownToolNames) {
		if (validTools.includes(toolName)) {
			continue; // Skip valid tools - these would have been parsed
		}

		const openTag = `<${toolName}>`;
		const closeTag = `</${toolName}>`;

		// Check if this tool tag exists
		const openIndex = xmlString.indexOf(openTag);
		if (openIndex === -1) {
			continue;
		}

		// Check if this tag is nested inside a valid tool tag
		let isNested = false;
		for (const validTool of validTools) {
			const validOpenTag = `<${validTool}>`;
			const validCloseTag = `</${validTool}>`;
			const validOpenIndex = xmlString.indexOf(validOpenTag);
			const validCloseIndex = xmlString.indexOf(validCloseTag);

			// If the unrecognized tool tag is between a valid tool's open and close tags, it's nested (a parameter)
			if (validOpenIndex !== -1 && validCloseIndex !== -1 &&
			    validOpenIndex < openIndex && openIndex < validCloseIndex) {
				isNested = true;
				break;
			}
		}

		if (!isNested) {
			return toolName;
		}
	}

	// Check if any valid tool name appears inside specific wrapper patterns
	// This catches cases where AI wraps tools in arbitrary tags like:
	// <api_call><tool_name>attempt_completion</tool_name>...</api_call>
	// <function>search</function>
	// <call name="extract">...</call>
	// Only match specific wrapper patterns to avoid false positives with normal text
	const allToolNames = [...new Set([...knownToolNames, ...validTools])];
	for (const toolName of allToolNames) {
		// Escape regex metacharacters in tool name to prevent regex errors
		const escapedToolName = toolName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

		// Match specific wrapper patterns that indicate a tool call attempt:
		// 1. <tool_name>toolName</tool_name> - common Claude API-style wrapper
		// 2. <function>toolName</function> - function call style
		// 3. <name>toolName</name> - generic name wrapper
		// 4. <call><name>toolName - partial wrapper patterns
		const wrapperPatterns = [
			new RegExp(`<tool_name>\\s*${escapedToolName}\\s*</tool_name>`, 'i'),
			new RegExp(`<function>\\s*${escapedToolName}\\s*</function>`, 'i'),
			new RegExp(`<name>\\s*${escapedToolName}\\s*</name>`, 'i'),
			// Also check for tool name immediately after api_call or call opening tag
			new RegExp(`<(?:api_call|call)[^>]*>[\\s\\S]*?<tool_name>\\s*${escapedToolName}`, 'i')
		];

		for (const pattern of wrapperPatterns) {
			if (pattern.test(xmlString)) {
				return `wrapped_tool:${toolName}`;
			}
		}
	}

	return null;
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

/**
 * Parse and resolve paths from a comma-separated string
 * Handles both relative and absolute paths, resolving relative paths against the cwd
 *
 * @param {string} pathStr - Path string, possibly comma-separated
 * @param {string} cwd - Working directory for resolving relative paths
 * @returns {string[]} Array of resolved paths
 */
export function parseAndResolvePaths(pathStr, cwd) {
	if (!pathStr) return [];

	// Split on comma and trim whitespace
	const paths = pathStr.split(',').map(p => p.trim()).filter(p => p.length > 0);

	// Resolve relative paths against cwd
	return paths.map(p => {
		if (isAbsolute(p)) {
			return p;
		}
		// Resolve relative path against cwd
		return cwd ? resolve(cwd, p) : p;
	});
}

/**
 * Resolve a target path that may include line numbers or symbols
 * Handles formats: "file.rs", "file.rs:10", "file.rs:10-20", "file.rs#symbol"
 * On Windows, correctly handles drive letter colons (e.g., "C:\path\file.rs:42")
 *
 * @param {string} target - Target string with optional line number or symbol
 * @param {string} cwd - Working directory for resolving relative paths
 * @returns {string} Resolved target path with suffix preserved
 */
export function resolveTargetPath(target, cwd) {
	// On Windows, skip the drive letter colon (e.g., "C:" at index 1)
	const searchStart = (target.length > 2 && target[1] === ':' && /[a-zA-Z]/.test(target[0])) ? 2 : 0;
	const colonIdx = target.indexOf(':', searchStart);
	const hashIdx = target.indexOf('#');
	let filePart, suffix;

	if (colonIdx !== -1 && (hashIdx === -1 || colonIdx < hashIdx)) {
		// Has line number (file.rs:10 or file.rs:10-20)
		filePart = target.substring(0, colonIdx);
		suffix = target.substring(colonIdx);
	} else if (hashIdx !== -1) {
		// Has symbol (file.rs#symbol)
		filePart = target.substring(0, hashIdx);
		suffix = target.substring(hashIdx);
	} else {
		// Just file path
		filePart = target;
		suffix = '';
	}

	// Resolve relative path
	if (!isAbsolute(filePart) && cwd) {
		filePart = resolve(cwd, filePart);
	}

	return filePart + suffix;
}
