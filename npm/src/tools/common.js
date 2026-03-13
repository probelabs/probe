/**
 * Common schemas and definitions for AI tools
 * @module tools/common
 */

import { z } from 'zod';
import { resolve, isAbsolute } from 'path';

// Common schemas for tool parameters (used for internal execution after XML parsing)
export const searchSchema = z.object({
	query: z.string().describe('Search query — natural language questions or Elasticsearch-style keywords both work. For keywords: use quotes for exact phrases, AND/OR for boolean logic, - for negation. Probe handles stemming and camelCase/snake_case splitting automatically, so do NOT try case or style variations of the same keyword.'),
	path: z.string().optional().default('.').describe('Path to search in. For dependencies use "go:github.com/owner/repo", "js:package_name", or "rust:cargo_name" etc.'),
	exact: z.boolean().optional().default(false).describe('Default (false) enables stemming and keyword splitting for exploratory search - "getUserData" matches "get", "user", "data", etc. Set true for precise symbol lookup OR when searching for strings with punctuation/quotes/empty values (e.g. \'description: ""\' — BM25 strips punctuation so exact=true is required for literal matching). Use true when you know the exact symbol name or need literal string matching.'),
	maxTokens: z.number().nullable().optional().describe('Maximum tokens to return. Default is 20000. Set to null for unlimited results.'),
	session: z.string().optional().describe('Session ID for result caching and pagination. Pass the session ID from a previous search to get additional results (next page). Results already shown in a session are automatically excluded. Omit for a fresh search.'),
	nextPage: z.boolean().optional().default(false).describe('Set to true when requesting the next page of results. Requires passing the same session ID from the previous search output.')
});

export const searchAllSchema = z.object({
	query: z.string().describe('Search query — natural language questions or Elasticsearch-style keywords both work. For keywords: use quotes for exact phrases, AND/OR for boolean logic, - for negation. Probe handles stemming and camelCase/snake_case splitting automatically, so do NOT try case or style variations of the same keyword.'),
	path: z.string().optional().default('.').describe('Path to search in.'),
	exact: z.boolean().optional().default(false).describe('Use exact matching instead of stemming.'),
	maxTokensPerPage: z.number().optional().default(20000).describe('Tokens per page when paginating. Default 20000.'),
	maxPages: z.number().optional().default(50).describe('Maximum pages to retrieve. Default 50 (safety limit).')
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

export const listFilesSchema = z.object({
	directory: z.string().optional().describe('Directory to list files from. Defaults to current directory.')
});

export const searchFilesSchema = z.object({
	pattern: z.string().describe('Glob pattern to search for (e.g., "**/*.js", "*.md")'),
	directory: z.string().optional().describe('Directory to search in. Defaults to current directory.'),
	recursive: z.boolean().optional().default(true).describe('Whether to search recursively')
});

export const readImageSchema = z.object({
	path: z.string().describe('Path to the image file to read. Supports png, jpg, jpeg, webp, bmp, and svg formats.')
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

export const cleanupExecutePlanSchema = z.object({
	clearOutputBuffer: z.boolean().optional().default(true).describe('Clear the output buffer from previous execute_plan calls'),
	clearSessionStore: z.boolean().optional().default(false).describe('Clear the session store (persisted data across execute_plan calls)')
});

// Legacy: attemptCompletionSchema kept as no-op for backward compatibility of external imports
// The attempt_completion tool has been removed — the model now completes naturally via text.
export const attemptCompletionSchema = {
	safeParse: (params) => {
		if (!params || typeof params !== 'object' || typeof params.result !== 'string') {
			return { success: false, error: { issues: [{ code: 'invalid_type', path: ['result'], message: 'Deprecated' }] } };
		}
		return { success: true, data: { result: params.result } };
	}
};


// Tool descriptions (used by Vercel tool() definitions)

export const searchDescription = 'Search code in the repository. Free-form questions are accepted, but Elasticsearch-style keyword queries work best. Use this tool first for any code-related questions. NOTE: By default, search handles stemming, case-insensitive matching, and camelCase/snake_case splitting automatically — do NOT manually try keyword variations like "getAllUsers" then "get_all_users" then "GetAllUsers". One search covers all variations.';
export const searchDelegateDescription = 'Search code in the repository by asking a question. Accepts natural language questions (e.g., "How does authentication work?", "Where is the user validation logic?"). A specialized subagent breaks down your question into targeted keyword searches and returns extracted code blocks. Do NOT formulate keyword queries yourself — just ask the question naturally.';
export const queryDescription = 'Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.';
export const extractDescription = 'Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files. Line numbers from output can be used with edit start_line/end_line for precise editing.';
export const delegateDescription = 'Automatically delegate big distinct tasks to specialized probe subagents within the agentic loop. Used by AI agents to break down complex requests into focused, parallel tasks.';
export const bashDescription = 'Execute bash commands for system operations: building, running tests, git, package management, etc. NEVER use for code exploration (no grep, cat, find, head, tail) — use search and extract tools instead. Secure by default with built-in allow/deny lists.';
export const analyzeAllDescription = 'Answer questions that require analyzing ALL matching data in the codebase. Use for aggregate questions like "What features exist?", "List all API endpoints", "Count TODO comments". The AI automatically plans the search strategy, processes all results via map-reduce, and synthesizes a comprehensive answer. WARNING: Slower than search - only use when you need complete coverage.';


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
 * Detect if a response indicates the agent is stuck in a loop or unable to proceed.
 *
 * @param {string} response - The agent's text response
 * @returns {boolean} - True if the response indicates a stuck state
 */
export function detectStuckResponse(response) {
	if (!response || typeof response !== 'string') {
		return false;
	}

	const stuckPatterns = [
		/\bi\s+cannot\s+proceed\b/i,
		/\bi\s+can['']t\s+(?:proceed|continue|move\s+forward)\b/i,
		/\bunable\s+to\s+(?:proceed|continue|complete)\b/i,
		/\bblocked\b.*\b(?:proceed|continue)\b/i,
		/\bneed\s+(?:the|an?)\s+\w+(?:\s+\w+)?\s+to\s+(?:proceed|continue)\b/i,
		/\brequire[sd]?\s+(?:the|an?)\s+\w+\b.*\bto\s+(?:proceed|continue)\b/i,
		/\bmissing\s+(?:required|necessary|essential)\b/i,
		/\bdeadlock\b/i,
		/\bwe\s+are\s+in\s+a\s+loop\b/i,
		/\bstuck\s+in\s+a\s+loop\b/i,
		/\bi\s+(?:have|['']ve)\s+(?:explained|stated|mentioned)\s+(?:this|the\s+situation|it)\s+(?:multiple|several)\s+times\b/i,
		/\bi\s+(?:cannot|can['']t|could\s+not|couldn['']t)\s+(?:find|locate|get|retrieve|obtain)\s+(?:the|this|that|an?)\b/i,
		/\bno\s+way\s+to\s+(?:find|get|obtain|retrieve)\b/i,
		/\bi\s+(?:have|['']ve)\s+exhausted\s+(?:all|my)\s+(?:available\s+)?(?:options|methods|approaches)\b/i,
		/\bneither\s+of\s+these\s+methods\b/i,
	];

	for (const pattern of stuckPatterns) {
		if (pattern.test(response)) {
			return true;
		}
	}

	return false;
}

/**
 * Check if two responses both indicate a stuck state.
 *
 * @param {string} response1 - First response
 * @param {string} response2 - Second response
 * @returns {boolean} - True if both responses indicate a stuck state
 */
export function areBothStuckResponses(response1, response2) {
	return detectStuckResponse(response1) && detectStuckResponse(response2);
}


/**
 * Parse a shell-like string into tokens, respecting quoted substrings.
 * Supports double quotes, single quotes, and escaped characters within quotes.
 * Splits on commas and/or whitespace outside of quotes.
 *
 * @param {string} input - The string to tokenize
 * @returns {string[]} Array of tokens with quotes stripped
 *
 * @example
 * splitQuotedString('"path with spaces/file.md" other.rs')
 * // Returns: ["path with spaces/file.md", "other.rs"]
 */
export function splitQuotedString(input) {
	const tokens = [];
	let current = '';
	let inQuote = null; // null, '"', or "'"
	let i = 0;

	while (i < input.length) {
		const ch = input[i];

		if (inQuote) {
			if (ch === '\\' && i + 1 < input.length) {
				// Escaped character inside quotes — keep the literal character
				current += input[i + 1];
				i += 2;
				continue;
			}
			if (ch === inQuote) {
				// Closing quote
				inQuote = null;
				i++;
				continue;
			}
			current += ch;
			i++;
		} else {
			if (ch === '"' || ch === "'") {
				inQuote = ch;
				i++;
				continue;
			}
			if (/[\s,]/.test(ch)) {
				// Delimiter outside quotes
				if (current.length > 0) {
					tokens.push(current);
					current = '';
				}
				i++;
				continue;
			}
			current += ch;
			i++;
		}
	}

	if (current.length > 0) {
		tokens.push(current);
	}

	return tokens;
}

/**
 * Parse targets string into array of file specifications
 * Handles both space-separated and comma-separated targets for extract tool.
 * Quoted strings (single or double) are preserved as single targets,
 * allowing file paths with spaces.
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
 * parseTargets('"Customers/First American/Meeting Notes.md" other.rs')
 * // Returns: ["Customers/First American/Meeting Notes.md", "other.rs"]
 */
export function parseTargets(targets) {
	if (!targets || typeof targets !== 'string') {
		return [];
	}

	return splitQuotedString(targets);
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

	// If the input contains quotes, use the quote-aware tokenizer which
	// preserves quoted strings with spaces as single tokens.
	if (/["']/.test(pathStr)) {
		const paths = splitQuotedString(pathStr);
		return paths.map(p => {
			if (isAbsolute(p)) return p;
			return cwd ? resolve(cwd, p) : p;
		});
	}

	// No quotes: use comma-split + space-split heuristic (original behavior).
	// Split on comma first, then auto-fix space-separated paths if each part
	// looks like a file path.
	let paths = pathStr.split(',').map(p => p.trim()).filter(p => p.length > 0);

	// Auto-fix: model sometimes passes space-separated file paths as one string
	// e.g. "src/ranking.rs src/simd_ranking.rs" — split if each part looks like a path
	paths = paths.flatMap(p => {
		if (!/\s/.test(p)) return [p];
		const parts = p.split(/\s+/).filter(Boolean);
		if (parts.length <= 1) return [p];
		const allLookLikePaths = parts.every(part => /[/\\]/.test(part) || /\.\w+/.test(part));
		return allLookLikePaths ? parts : [p];
	});

	// Resolve relative paths against cwd
	return paths.map(p => {
		if (isAbsolute(p)) return p;
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
