/**
 * Tools for Vercel AI SDK
 * @module tools/vercel
 */

import { tool } from 'ai';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { delegate } from '../delegate.js';
import { searchSchema, querySchema, extractSchema, delegateSchema, searchDescription, queryDescription, extractDescription, delegateDescription, parseTargets, parseAndResolvePaths, resolveTargetPath } from './common.js';

const CODE_SEARCH_SCHEMA = {
	type: 'object',
	properties: {
		targets: {
			type: 'array',
			items: { type: 'string' },
			description: 'List of file targets like "path/to/file.ext#Symbol" or "path/to/file.ext:line" or "path/to/file.ext:start-end".'
		}
	},
	required: ['targets'],
	additionalProperties: false
};

function normalizeTargets(targets) {
	if (!Array.isArray(targets)) return [];
	const seen = new Set();
	const normalized = [];

	for (const target of targets) {
		if (typeof target !== 'string') continue;
		const trimmed = target.trim();
		if (!trimmed || seen.has(trimmed)) continue;
		seen.add(trimmed);
		normalized.push(trimmed);
	}

	return normalized;
}

function extractJsonSnippet(text) {
	const jsonBlockMatch = text.match(/```json\s*([\s\S]*?)```/i);
	if (jsonBlockMatch) {
		return jsonBlockMatch[1].trim();
	}

	const anyBlockMatch = text.match(/```\s*([\s\S]*?)```/);
	if (anyBlockMatch) {
		return anyBlockMatch[1].trim();
	}

	const firstBrace = text.indexOf('{');
	const lastBrace = text.lastIndexOf('}');
	if (firstBrace !== -1 && lastBrace > firstBrace) {
		return text.slice(firstBrace, lastBrace + 1);
	}

	const firstBracket = text.indexOf('[');
	const lastBracket = text.lastIndexOf(']');
	if (firstBracket !== -1 && lastBracket > firstBracket) {
		return text.slice(firstBracket, lastBracket + 1);
	}

	return null;
}

function fallbackTargetsFromText(text) {
	const candidates = [];
	const lines = text.split(/\r?\n/);

	for (const line of lines) {
		let cleaned = line.trim();
		if (!cleaned) continue;
		cleaned = cleaned.replace(/^[-*•\d.)\s]+/, '').trim();
		if (!cleaned) continue;
		const token = cleaned.split(/\s+/)[0];
		if (/[#:]|[/\\]|\\./.test(token)) {
			candidates.push(token);
		}
	}

	return candidates;
}

function parseDelegatedTargets(rawResponse) {
	if (!rawResponse || typeof rawResponse !== 'string') return [];
	const trimmed = rawResponse.trim();

	const tryParse = (text) => {
		try {
			return JSON.parse(text);
		} catch {
			return null;
		}
	};

	let parsed = tryParse(trimmed);
	if (!parsed) {
		const snippet = extractJsonSnippet(trimmed);
		if (snippet) {
			parsed = tryParse(snippet);
		}
	}

	if (parsed) {
		if (Array.isArray(parsed)) {
			return normalizeTargets(parsed);
		}
		if (Array.isArray(parsed.targets)) {
			return normalizeTargets(parsed.targets);
		}
	}

	return normalizeTargets(fallbackTargetsFromText(trimmed));
}

function buildSearchDelegateTask({ searchQuery, searchPath, exact, language, allowTests }) {
	return [
		'You are a code-search subagent. Your ONLY job is to return ALL relevant code locations.',
		'Use ONLY the search tool. Do NOT answer the question or explain anything.',
		'Use the query exactly as provided (no substitutions or paraphrasing). If nothing is found, return {"targets": []}.',
		'Return ONLY valid JSON with this shape: {"targets": ["path/to/file.ext#Symbol", "path/to/file.ext:line", "path/to/file.ext:start-end"]}.',
		'Prefer #Symbol when a function/class name is clear; otherwise use line numbers.',
		`Search query: ${searchQuery}`,
		`Search path(s): ${searchPath}`,
		`Options: exact=${exact ? 'true' : 'false'}, language=${language || 'auto'}, allow_tests=${allowTests ? 'true' : 'false'}.`,
		'Run additional searches only if needed to capture all relevant locations.',
		'Deduplicate targets.'
	].join('\n');
}

function buildDelegatedQuestion(query) {
	if (!query || typeof query !== 'string') return query;
	const trimmed = query.trim();
	if (!trimmed) return trimmed;
	const looksQuestion = /[?]$/.test(trimmed) || /^(how|where|what|why|when|which|explain|describe|show|find)\b/i.test(trimmed);
	const hasSpace = /\s/.test(trimmed);
	const hasOperators = /\b(AND|OR|NOT)\b/i.test(trimmed);
	const hasPunctuation = /[":()]/.test(trimmed);

	if (looksQuestion) return trimmed;
	if (!hasSpace && !hasOperators && !hasPunctuation) {
		return `Where is "${trimmed}" defined or used in the codebase?`;
	}
	return trimmed;
}

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
	const {
		sessionId,
		maxTokens = 10000,
		debug = false,
		outline = false,
		searchDelegate = false
	} = options;
	const withSpan = options.tracer?.withSpan
		? (name, fn, attrs) => options.tracer.withSpan(name, fn, attrs)
		: (_name, fn) => fn();

	return tool({
		name: 'search',
		description: searchDelegate
			? `${searchDescription} (delegates code search to a subagent and returns extracted code blocks)`
			: searchDescription,
		inputSchema: searchSchema,
		execute: async ({ query: searchQuery, path, allow_tests, exact, maxTokens: paramMaxTokens, language }) => {
			// Use parameter maxTokens if provided, otherwise use the default
			const effectiveMaxTokens = paramMaxTokens || maxTokens;

			// Parse and resolve paths (supports comma-separated and relative paths)
			let searchPaths;
			if (path) {
				searchPaths = parseAndResolvePaths(path, options.cwd);
			}

			// Default to cwd or '.' if no paths provided
			if (!searchPaths || searchPaths.length === 0) {
				searchPaths = [options.cwd || '.'];
			}

			// Join paths with space for CLI (probe search supports multiple paths)
			const searchPath = searchPaths.join(' ');

			const searchOptions = {
				query: searchQuery,
				path: searchPath,
				cwd: options.cwd, // Working directory for resolving relative paths
				allowTests: allow_tests ?? true,
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

			const runRawSearch = async () => withSpan('search.raw', async () => {
				if (debug) {
					console.error(`Executing search with query: "${searchQuery}", path: "${searchPath}", exact: ${exact ? 'true' : 'false'}, language: ${language || 'all'}, session: ${sessionId || 'none'}`);
				}
				return await search(searchOptions);
			}, {
				'search.query': searchQuery,
				'search.path': searchPath,
				'search.exact': Boolean(exact),
				'probe.search.query': searchQuery,
				'probe.search.path': searchPath,
				'probe.search.exact': Boolean(exact),
				'probe.search.max_tokens': effectiveMaxTokens
			});

			if (!searchDelegate) {
				try {
					return await runRawSearch();
				} catch (error) {
					console.error('Error executing search command:', error);
					return `Error executing search command: ${error.message}`;
				}
			}

			try {
				if (debug) {
					console.error(`Delegating search with query: "${searchQuery}", path: "${searchPath}"`);
				}

				const delegatedQuery = buildDelegatedQuestion(searchQuery);
				const delegateTask = buildSearchDelegateTask({
					searchQuery: delegatedQuery,
					searchPath,
					exact,
					language,
					allowTests: allow_tests ?? true
				});

				const runDelegation = () => delegate({
					task: delegateTask,
					debug,
					parentSessionId: sessionId,
					path: options.allowedFolders?.[0] || options.cwd || '.',
					allowedFolders: options.allowedFolders,
					provider: options.provider || null,
					model: options.model || null,
					tracer: options.tracer || null,
					enableBash: false,
					bashConfig: null,
					architectureFileName: options.architectureFileName || null,
					promptType: 'code-searcher',
					allowedTools: ['search', 'attempt_completion'],
					searchDelegate: false,
					schema: CODE_SEARCH_SCHEMA
				});

				const delegateResult = await withSpan('search.delegate', runDelegation, {
					'search.query': delegatedQuery,
					'search.path': searchPath,
					'probe.search.query': delegatedQuery,
					'probe.search.path': searchPath,
					'probe.search.original_query': searchQuery
				});

				const targets = parseDelegatedTargets(delegateResult);
				if (!targets.length) {
					if (debug) {
						console.error('Delegated search returned no targets; falling back to raw search');
					}
					return await runRawSearch();
				}

				const effectiveCwd = options.cwd || '.';
				const resolvedTargets = targets.map(target => resolveTargetPath(target, effectiveCwd));
				const targetSampleLimit = 25;
				const targetSample = resolvedTargets.slice(0, targetSampleLimit);
				const targetSampleString = targetSample.join(', ');
				const extractOptions = {
					files: resolvedTargets,
					cwd: effectiveCwd,
					allowTests: allow_tests ?? true
				};

				if (outline) {
					extractOptions.format = 'xml';
				}

				return await withSpan('search.extract', () => extract(extractOptions), {
					'search.targets': resolvedTargets.length,
					'search.path': searchPath,
					'probe.search.targets': resolvedTargets.length,
					'probe.search.path': searchPath,
					'probe.search.targets_sample': targetSampleString,
					'probe.search.targets_sample_count': targetSample.length,
					'probe.search.targets_truncated': resolvedTargets.length > targetSampleLimit
				});
			} catch (error) {
				console.error('Delegated search failed, falling back to raw search:', error);
				try {
					return await runRawSearch();
				} catch (fallbackError) {
					console.error('Error executing search command:', fallbackError);
					return `Error executing search command: ${fallbackError.message}`;
				}
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
				// Parse and resolve paths (supports comma-separated and relative paths)
				let queryPaths;
				if (path) {
					queryPaths = parseAndResolvePaths(path, options.cwd);
				}

				// Default to cwd or '.' if no paths provided
				if (!queryPaths || queryPaths.length === 0) {
					queryPaths = [options.cwd || '.'];
				}

				// Join paths with space for CLI (probe query supports multiple paths)
				const queryPath = queryPaths.join(' ');

				if (debug) {
					console.error(`Executing query with pattern: "${pattern}", path: "${queryPath}", language: ${language || 'auto'}`);
				}

				const results = await query({
					pattern,
					path: queryPath,
					cwd: options.cwd, // Working directory for resolving relative paths
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
				// Use the cwd from config for working directory
				const effectiveCwd = options.cwd || '.';

				if (debug) {
					if (targets) {
						console.error(`Executing extract with targets: "${targets}", cwd: "${effectiveCwd}", context lines: ${context_lines || 10}`);
					} else if (input_content) {
						console.error(`Executing extract with input content, cwd: "${effectiveCwd}", context lines: ${context_lines || 10}`);
					}
				}

				// Create a temporary file for input content if provided
				let tempFilePath = null;
				let extractOptions = { cwd: effectiveCwd };

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
						cwd: effectiveCwd,
						allowTests: allow_tests ?? true,
						contextLines: context_lines,
						format: effectiveFormat
					};
				} else if (targets) {
					// Parse targets to handle line numbers and symbol names
					// Now supports both whitespace and comma-separated targets
					const parsedTargets = parseTargets(targets);

					// Resolve relative paths in targets against cwd
					const files = parsedTargets.map(target => resolveTargetPath(target, effectiveCwd));

					// Apply format mapping for outline-xml to xml
					let effectiveFormat = format;
					if (outline && format === 'outline-xml') {
						effectiveFormat = 'xml';
					}

					// Set up extract options with files
					extractOptions = {
						files,
						cwd: effectiveCwd,
						allowTests: allow_tests ?? true,
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
 * @param {string} [options.cwd] - Working directory to use if not specified in call
 * @param {string[]} [options.allowedFolders] - Allowed folders for workspace isolation
 * @param {boolean} [options.enableBash=false] - Enable bash tool for sub-agents
 * @param {Object} [options.bashConfig] - Bash configuration (allow/deny patterns)
 * @param {string} [options.architectureFileName] - Architecture context filename to embed from repo root
 * @returns {Object} Configured delegate tool
 */
export const delegateTool = (options = {}) => {
	const { debug = false, timeout = 300, cwd, allowedFolders, enableBash = false, bashConfig, architectureFileName } = options;

	return tool({
		name: 'delegate',
		description: delegateDescription,
		inputSchema: delegateSchema,
		execute: async ({ task, currentIteration, maxIterations, parentSessionId, path, provider, model, tracer, searchDelegate }) => {
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

			if (searchDelegate !== undefined && typeof searchDelegate !== 'boolean') {
				throw new TypeError('searchDelegate must be a boolean if provided');
			}

			// Determine the path to pass to the subagent
			// NOTE: Delegation intentionally uses DIFFERENT priority than other tools.
			//
			// Other tools (search, extract, query, bash) use: cwd || allowedFolders[0]
			// Delegation uses: path || allowedFolders[0] || cwd
			//
			// This is intentional because:
			// - Other tools operate within the parent's navigation context (cwd is correct)
			// - Subagents need a FRESH start from workspace root, not parent's navigation state
			// - Using parent's cwd would cause "path doubling" (Issue #348) where paths like
			//   /workspace/project/src/internal/build/src/internal/build/file.go get constructed
			//
			// The workspace root (allowedFolders[0]) is the security boundary and correct base
			// for subagent operations. Parent navigation context should not leak to subagents.
			const workspaceRoot = allowedFolders && allowedFolders[0];
			const effectivePath = path || workspaceRoot || cwd;

			if (debug) {
				console.error(`Executing delegate with task: "${task.substring(0, 100)}${task.length > 100 ? '...' : ''}"`);
				if (parentSessionId) {
					console.error(`Parent session: ${parentSessionId}`);
				}
				if (effectivePath && effectivePath !== path) {
					console.error(`Using workspace root: ${effectivePath} (cwd was: ${cwd || 'not set'})`);
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
				allowedFolders,
				provider,
				model,
				tracer,
				enableBash,
				bashConfig,
				architectureFileName,
				searchDelegate
			});

			return result;
		}
	});
};
