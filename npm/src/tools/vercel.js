/**
 * Tools for Vercel AI SDK
 * @module tools/vercel
 */

import { tool } from 'ai';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { delegate } from '../delegate.js';
import { analyzeAll } from './analyzeAll.js';
import { searchSchema, querySchema, extractSchema, delegateSchema, analyzeAllSchema, searchDescription, searchDelegateDescription, queryDescription, extractDescription, delegateDescription, analyzeAllDescription, parseTargets, parseAndResolvePaths, resolveTargetPath } from './common.js';
import { existsSync } from 'fs';
import { formatErrorForAI } from '../utils/error-types.js';
import { annotateOutputWithHashes } from './hashline.js';

/**
 * Auto-quote search query terms that contain mixed case or underscores.
 * Unquoted camelCase like "limitDRL" gets split by stemming into "limit" + "DRL".
 * This wraps such terms in quotes so they match as literal strings.
 *
 * Examples:
 *   "limitDRL limitRedis"    → '"limitDRL" "limitRedis"'
 *   "ThrottleRetryLimit"     → '"ThrottleRetryLimit"'
 *   "allowed_ips"            → '"allowed_ips"'
 *   "rate limit"             → 'rate limit'  (no change, all lowercase)
 *   '"already quoted"'       → '"already quoted"'  (no change)
 *   'foo AND bar'            → 'foo AND bar'  (operators preserved)
 */
function autoQuoteSearchTerms(query) {
	if (!query || typeof query !== 'string') return query;

	// Split on whitespace, preserving quoted strings and operators
	const tokens = [];
	let i = 0;
	while (i < query.length) {
		// Skip whitespace
		if (/\s/.test(query[i])) {
			i++;
			continue;
		}
		// Quoted string — keep as-is
		if (query[i] === '"') {
			const end = query.indexOf('"', i + 1);
			if (end !== -1) {
				tokens.push(query.substring(i, end + 1));
				i = end + 1;
			} else {
				// Unclosed quote — take rest
				tokens.push(query.substring(i));
				break;
			}
			continue;
		}
		// Unquoted token
		let j = i;
		while (j < query.length && !/\s/.test(query[j]) && query[j] !== '"') {
			j++;
		}
		tokens.push(query.substring(i, j));
		i = j;
	}

	// Boolean operators that should not be quoted
	const operators = new Set(['AND', 'OR', 'NOT']);

	const result = tokens.map(token => {
		// Already quoted
		if (token.startsWith('"')) return token;
		// Boolean operator
		if (operators.has(token)) return token;
		// Check if token needs quoting: has camelCase/PascalCase transitions or underscores
		// Simple capitalized words like "Redis" or "Limiter" should NOT be quoted —
		// only quote when there's an actual case transition (e.g., "getUserData", "NewSlidingLog")
		const hasUnderscore = token.includes('_');
		const hasCaseTransition = /[a-z][A-Z]/.test(token) || /[A-Z]{2,}[a-z]/.test(token);
		if (hasCaseTransition || hasUnderscore) {
			return `"${token}"`;
		}
		return token;
	});

	return result.join(' ');
}

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

		// Auto-fix: model sometimes puts multiple space-separated file paths in one string
		// e.g. "src/ranking.rs src/simd_ranking.rs" — split them apart
		const subTargets = splitSpaceSeparatedPaths(trimmed);
		for (const sub of subTargets) {
			if (!seen.has(sub)) {
				seen.add(sub);
				normalized.push(sub);
			}
		}
	}

	return normalized;
}

/**
 * Split a string that may contain multiple space-separated file paths.
 * Detects patterns like "path/file.ext path2/file2.ext" and splits them.
 * Preserves single paths and paths with suffixes like ":10-20" or "#Symbol".
 */
function splitSpaceSeparatedPaths(target) {
	// If no spaces, it's a single target
	if (!/\s/.test(target)) return [target];

	// Split on whitespace and check if parts look like file paths
	const parts = target.split(/\s+/).filter(Boolean);
	if (parts.length <= 1) return [target];

	// Check if each part looks like a file path (has a dot extension or path separator)
	const allLookLikePaths = parts.every(p => /[/\\]/.test(p) || /\.\w+/.test(p));
	if (allLookLikePaths) return parts;

	// Not confident these are separate paths — return as-is
	return [target];
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

function splitTargetSuffix(target) {
	const searchStart = (target.length > 2 && target[1] === ':' && /[a-zA-Z]/.test(target[0])) ? 2 : 0;
	const colonIdx = target.indexOf(':', searchStart);
	const hashIdx = target.indexOf('#');
	if (colonIdx !== -1 && (hashIdx === -1 || colonIdx < hashIdx)) {
		return { filePart: target.substring(0, colonIdx), suffix: target.substring(colonIdx) };
	} else if (hashIdx !== -1) {
		return { filePart: target.substring(0, hashIdx), suffix: target.substring(hashIdx) };
	}
	return { filePart: target, suffix: '' };
}

function buildSearchDelegateTask({ searchQuery, searchPath, exact, language, allowTests }) {
	return [
		'You are a code-search subagent. Your job is to find ALL relevant code locations for the given query.',
		'',
		'The query may be complex - it could be a natural language question, a multi-part request, or a simple keyword.',
		'Break down complex queries into multiple searches to cover all aspects.',
		'',
		'Available tools:',
		'- search: Find code matching keywords or patterns. Run multiple searches for different aspects of complex queries.',
		'- extract: Verify code snippets to ensure targets are actually relevant before including them.',
		'- listFiles: Understand directory structure to find where relevant code might live.',
		'',
		'CRITICAL - How probe search works (do NOT ignore):',
		'- By default (exact=false), probe ALREADY handles stemming, case-insensitive matching, and camelCase/snake_case splitting automatically.',
		'- Searching "allowed_ips" ALREADY matches "AllowedIPs", "allowedIps", "allowed_ips", etc. Do NOT manually try case/style variations.',
		'- Searching "getUserData" ALREADY matches "get", "user", "data" and their variations.',
		'- NEVER repeat the same search query — you will get the same results. Changing the path does NOT change this.',
		'- NEVER search trivial variations of the same keyword (e.g., AllowedIPs then allowedIps then allowed_ips). This is wasteful — probe handles it.',
		'- If a search returns no results, the term likely does not exist. Try a genuinely DIFFERENT keyword or concept, not a variation.',
		'- If 2-3 searches return no results for a concept, STOP searching for it and move on. Do NOT keep retrying.',
		'',
		'When to use exact=true:',
		'- Use exact=true when searching for a KNOWN symbol name (function, type, variable, struct).',
		'- exact=true matches the literal string only — no stemming, no splitting.',
		'- This is ideal for precise lookups: exact=true "ForwardMessage", exact=true "SessionLimiter", exact=true "ThrottleRetryLimit".',
		'- Do NOT use exact=true for exploratory/conceptual queries — use the default for those.',
		'',
		'Combining searches with OR:',
		'- Multiple unquoted words use OR logic: rate limit matches files containing EITHER "rate" OR "limit".',
		'- IMPORTANT: Multiple quoted terms use AND logic by default: \'"RateLimit" "middleware"\' requires BOTH in the same file.',
		'- To search for ANY of several quoted symbols, use the explicit OR operator: \'"ForwardMessage" OR "SessionLimiter"\'.',
		'- Without quotes, camelCase like limitDRL gets split into "limit" + "DRL" — not what you want for symbol lookup.',
		'- Use OR to search for multiple related symbols in ONE search instead of separate searches.',
		'- This is much faster than running separate searches sequentially.',
		'- Example: search \'"ForwardMessage" OR "SessionLimiter"\' finds files with either exact symbol in one call.',
		'- Example: search \'"limitDRL" OR "doRollingWindowWrite"\' finds both rate limiting functions at once.',
		'- Use AND (or just put quoted terms together) when you need both terms in the same file.',
		'',
		'Parallel tool calls:',
		'- When you need to search for INDEPENDENT concepts, call multiple search tools IN PARALLEL (same response).',
		'- Do NOT wait for one search to finish before starting the next if they are independent.',
		'- Example: for "rate limiting and session management", call search "rate limiting" AND search "session management" in parallel.',
		'- Similarly, call multiple extract tools in parallel when verifying different files.',
		'',
		'GOOD search strategy (do this):',
		'  Query: "How does authentication work and how are sessions managed?"',
		'  → search "authentication" + search "session management" IN PARALLEL (two independent concepts)',
		'  Query: "Find the IP allowlist middleware"',
		'  → search "allowlist middleware" (one search, probe handles IP/ip/Ip variations)',
		'  Query: "Find ForwardMessage and SessionLimiter"',
		'  → search \'"ForwardMessage" OR "SessionLimiter"\' (one OR search finds both exact symbols)',
		'  OR: search exact=true "ForwardMessage" + search exact=true "SessionLimiter" IN PARALLEL',
		'  Query: "Find limitDRL and limitRedis functions"',
		'  → search \'"limitDRL" OR "limitRedis"\' (one OR search, quoted to prevent camelCase splitting)',
		'  Query: "Find ThrottleRetryLimit usage"',
		'  → search exact=true "ThrottleRetryLimit" (one search, if no results the symbol does not exist — stop)',
		'  Query: "How does BM25 scoring work with SIMD optimization?"',
		'  → search "BM25 scoring" + search "SIMD optimization" IN PARALLEL (two different concepts)',
		'',
		'BAD search strategy (never do this):',
		'  → search "AllowedIPs" → search "allowedIps" → search "allowed_ips" (WRONG: case/style variations, probe handles them)',
		'  → search "limitDRL" → search "LimitDRL" (WRONG: case variation — combine with OR: \'"limitDRL" OR "limitRedis"\')',
		'  → search "throttle_retry_limit" after searching "ThrottleRetryLimit" (WRONG: snake_case variation, probe handles it)',
		'  → search "ThrottleRetryLimit" path=tyk → search "ThrottleRetryLimit" path=gateway → search "ThrottleRetryLimit" path=apidef (WRONG: same query on different paths — probe searches recursively)',
		'  → search "func (k *RateLimitAndQuotaCheck) handleRateLimitFailure" (WRONG: do not search full function signatures, just use exact=true "handleRateLimitFailure")',
		'  → search "ForwardMessage" → search "ForwardMessage" → search "ForwardMessage" (WRONG: repeating the exact same query)',
		'  → search "authentication" → wait → search "session management" → wait (WRONG: these are independent, run them in parallel)',
		'',
		'Keyword tips:',
		'- Common programming keywords are filtered as stopwords when unquoted: function, class, return, new, struct, impl, var, let, const, etc.',
		'- Avoid searching for these alone — combine with a specific term (e.g., "middleware function" is fine, "function" alone is too generic).',
		'- To bypass stopword filtering: wrap terms in quotes ("return", "struct") or set exact=true. Both disable stemming and splitting too.',
		'- camelCase terms are split: getUserData becomes "get", "user", "data" — so one search covers all naming styles.',
		'- Do NOT search for full function signatures like "func (r *Type) Method(args)". Just search for the method name with exact=true.',
		'- Do NOT search for file names (e.g., "sliding_log.go"). Use listFiles to discover files by name.',
		'',
		'WHEN TO STOP — read this carefully:',
		'- You have a LIMITED number of tool calls. Do NOT waste them on repeated or speculative searches.',
		'- If you get a "DUPLICATE SEARCH BLOCKED" message, do NOT retry. Move to extract or output your answer.',
		'- If you get 2+ blocked messages in a row, IMMEDIATELY output your final JSON answer.',
		'- Once you have found the key files and verified them with extract, STOP and output your answer.',
		'- You do NOT need to find every single reference. Focus on the most relevant code locations.',
		'- It is MUCH better to return 5 highly relevant targets than to waste iterations searching for more.',
		'',
		'Strategy:',
		'1. Analyze the query - identify key concepts and group related symbols',
		'2. Combine related symbols into OR searches: \'"symbolA" OR "symbolB"\' finds files with either',
		'3. Run INDEPENDENT searches in PARALLEL — do not wait for one to finish before starting another',
		'4. For known symbol names use exact=true. For concepts use default (exact=false).',
		'5. If a search returns results, use extract to verify relevance. Run multiple extracts in parallel too.',
		'6. If a search returns NO results, the term does not exist. Do NOT retry with variations, different paths, or longer strings. Move on.',
		'7. Once you have enough targets (typically 3-10), output your final JSON answer immediately.',
		'',
		`Query: ${searchQuery}`,
		`Search path(s): ${searchPath}`,
		`Options: exact=${exact ? 'true' : 'false'}, language=${language || 'auto'}, allow_tests=${allowTests ? 'true' : 'false'}.`,
		'',
		'Return ONLY valid JSON: {"targets": ["path/to/file.ext#Symbol", "path/to/file.ext:line", "path/to/file.ext:start-end"]}',
		'IMPORTANT: Use ABSOLUTE file paths in targets (e.g., "/full/path/to/file.ext#Symbol"). If you only have relative paths, make them relative to the search path above.',
		'Prefer #Symbol when a function/class name is clear; otherwise use line numbers.',
		'Deduplicate targets. Do NOT explain or answer - ONLY return the JSON targets.'
	].join('\n');
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
		maxTokens = 20000,
		debug = false,
		outline = false,
		searchDelegate = false,
		hashLines = false
	} = options;

	const maybeAnnotate = (result) => {
		if (hashLines && typeof result === 'string') {
			return annotateOutputWithHashes(result);
		}
		return result;
	};

	// Track previous non-paginated searches to detect and block duplicates
	const previousSearches = new Set();
	// Track how many times a duplicate search has been blocked (for escalating messages)
	let consecutiveDupBlocks = 0;
	// Track pagination counts per query to cap runaway pagination
	const paginationCounts = new Map();
	const MAX_PAGES_PER_QUERY = 3;

	return tool({
		name: 'search',
		description: searchDelegate
			? searchDelegateDescription
			: searchDescription,
		inputSchema: searchSchema,
		execute: async ({ query: searchQuery, path, allow_tests, exact, maxTokens: paramMaxTokens, language, session, nextPage }) => {
			// Auto-quote mixed-case and underscore terms to prevent unwanted stemming/splitting
			// Skip when exact=true since that already preserves the literal string
			if (!exact && searchQuery) {
				const originalQuery = searchQuery;
				searchQuery = autoQuoteSearchTerms(searchQuery);
				if (debug && searchQuery !== originalQuery) {
					console.error(`[search] Auto-quoted query: "${originalQuery}" → "${searchQuery}"`);
				}
			}

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
				session: session || sessionId, // Use explicit session param, or fall back to options sessionId
				nextPage, // Pass nextPage parameter for pagination
				language // Pass language parameter if provided
			};

			// Add outline format if enabled
			if (outline) {
				searchOptions.format = 'outline-xml';
			}

			const runRawSearch = async () => {
				if (debug) {
					console.error(`Executing search with query: "${searchQuery}", path: "${searchPath}", exact: ${exact ? 'true' : 'false'}, language: ${language || 'all'}, session: ${sessionId || 'none'}`);
				}
				return await search(searchOptions);
			};

			if (!searchDelegate) {
				// Block duplicate non-paginated searches (models sometimes repeat the exact same call)
				// Allow pagination: only nextPage=true is a legitimate repeat of the same query
				// Use query+exact as the key (ignore path) to prevent path-hopping evasion
				// where model searches same term on different subpaths hoping for different results
				const searchKey = `${searchQuery}::${exact || false}`;
				if (!nextPage) {
					if (previousSearches.has(searchKey)) {
						consecutiveDupBlocks++;
						if (debug) {
							console.error(`[DEDUP] Blocked duplicate search (${consecutiveDupBlocks}x): "${searchQuery}" (path: "${searchPath}")`);
						}
						if (consecutiveDupBlocks >= 3) {
							return 'STOP. You have been blocked ' + consecutiveDupBlocks + ' times for repeating searches. You MUST output your final JSON answer NOW with whatever targets you have found. Do NOT call any more tools.';
						}
						return 'DUPLICATE SEARCH BLOCKED (' + consecutiveDupBlocks + 'x). You already searched for this. Do NOT repeat — probe searches recursively across all paths. Either: (1) use extract on results you already found, (2) try a COMPLETELY different keyword, or (3) output your final answer NOW.';
					}
					previousSearches.add(searchKey);
					consecutiveDupBlocks = 0; // Reset on successful new search
					paginationCounts.set(searchKey, 0);
				} else {
					// Cap pagination to prevent runaway page-through of broad queries
					const pageCount = (paginationCounts.get(searchKey) || 0) + 1;
					paginationCounts.set(searchKey, pageCount);
					if (pageCount > MAX_PAGES_PER_QUERY) {
						if (debug) {
							console.error(`[DEDUP] Blocked excessive pagination (page ${pageCount}/${MAX_PAGES_PER_QUERY}): "${searchQuery}" in "${searchPath}"`);
						}
						return `PAGINATION LIMIT REACHED: You have already retrieved ${MAX_PAGES_PER_QUERY} pages of results for this query. You have enough results — use extract to examine specific files, or provide your final answer with your findings.`;
					}
				}
				try {
					const result = maybeAnnotate(await runRawSearch());
					// Track files found in search results for staleness detection
					if (options.fileTracker && typeof result === 'string') {
						options.fileTracker.trackFilesFromOutput(result, options.cwd || '.').catch(() => {});
					}
					return result;
				} catch (error) {
					console.error('Error executing search command:', error);
					return formatErrorForAI(error);
				}
			}

			try {
				if (debug) {
					console.error(`Delegating search with query: "${searchQuery}", path: "${searchPath}"`);
				}

				const delegateTask = buildSearchDelegateTask({
					searchQuery,
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
					provider: options.searchDelegateProvider || process.env.PROBE_SEARCH_DELEGATE_PROVIDER || options.provider || null,
					model: options.searchDelegateModel || process.env.PROBE_SEARCH_DELEGATE_MODEL || options.model || null,
					tracer: options.tracer || null,
					enableBash: false,
					bashConfig: null,
					architectureFileName: options.architectureFileName || null,
					promptType: 'code-searcher',
					allowedTools: ['search', 'extract', 'listFiles'],
					searchDelegate: false,
					schema: CODE_SEARCH_SCHEMA,
					parentAbortSignal: options.parentAbortSignal || null,
					maxIterations: 15  // Cap delegate to 15 iterations — good delegates finish in 3-6
				});

				const delegateResult = options.tracer?.withSpan
					? await options.tracer.withSpan('search.delegate', runDelegation, {
						'search.query': searchQuery,
						'search.path': searchPath
					})
					: await runDelegation();

				const targets = parseDelegatedTargets(delegateResult);
				if (!targets.length) {
					if (debug) {
						console.error('Delegated search returned no targets; falling back to raw search');
					}
					const fallbackResult = maybeAnnotate(await runRawSearch());
					if (options.fileTracker && typeof fallbackResult === 'string') {
						options.fileTracker.trackFilesFromOutput(fallbackResult, options.cwd || '.').catch(() => {});
					}
					return fallbackResult;
				}

				// The delegate runs from workspace root (allowedFolders[0] or cwd), NOT from searchPaths[0].
				// It returns paths relative to that workspace root. Resolve against the same base.
				const delegateBase = options.allowedFolders?.[0] || options.cwd || '.';
				const resolutionBase = searchPaths[0] || options.cwd || '.';
				const resolvedTargets = targets.map(target => resolveTargetPath(target, delegateBase));

				// Auto-fix: detect and repair invalid paths (doubled segments, AI hallucinations)
				const validatedTargets = [];
				for (const target of resolvedTargets) {
					const { filePart, suffix } = splitTargetSuffix(target);

					// 1. Path exists as-is
					if (existsSync(filePart)) {
						validatedTargets.push(target);
						continue;
					}

					// 2. Detect doubled directory segments: /ws/proj/proj/src → /ws/proj/src
					let fixed = false;
					const parts = filePart.split('/').filter(Boolean);
					for (let i = 0; i < parts.length - 1; i++) {
						if (parts[i] === parts[i + 1]) {
							const candidate = '/' + [...parts.slice(0, i), ...parts.slice(i + 1)].join('/');
							if (existsSync(candidate)) {
								validatedTargets.push(candidate + suffix);
								if (debug) console.error(`[search-delegate] Fixed doubled path segment: ${filePart} → ${candidate}`);
								fixed = true;
								break;
							}
						}
					}
					if (fixed) continue;

					// 3. Try resolving against alternative bases (searchPaths[0], cwd)
					for (const altBase of [resolutionBase, options.cwd].filter(Boolean)) {
						if (altBase === delegateBase) continue;
						const altResolved = resolveTargetPath(target, altBase);
						const { filePart: altFile } = splitTargetSuffix(altResolved);
						if (existsSync(altFile)) {
							validatedTargets.push(altResolved);
							if (debug) console.error(`[search-delegate] Resolved with alt base: ${filePart} → ${altFile}`);
							fixed = true;
							break;
						}
					}
					if (fixed) continue;

					// 4. Keep target anyway (probe binary will report the error)
					//    but log a warning
					if (debug) console.error(`[search-delegate] Warning: target may not exist: ${filePart}`);
					validatedTargets.push(target);
				}

				const extractOptions = {
					files: validatedTargets,
					cwd: resolutionBase,
					allowTests: allow_tests ?? true
				};

				if (outline) {
					extractOptions.format = 'xml';
				}

				const extractResult = await extract(extractOptions);

				// Strip workspace root prefix from extract output so paths are relative
				if (resolutionBase && typeof extractResult === 'string') {
					const wsPrefix = resolutionBase.endsWith('/') ? resolutionBase : resolutionBase + '/';
					return maybeAnnotate(extractResult.split(wsPrefix).join(''));
				}

				return maybeAnnotate(extractResult);
			} catch (error) {
				console.error('Delegated search failed, falling back to raw search:', error);
				try {
					const fallbackResult2 = maybeAnnotate(await runRawSearch());
					if (options.fileTracker && typeof fallbackResult2 === 'string') {
						options.fileTracker.trackFilesFromOutput(fallbackResult2, options.cwd || '.').catch(() => {});
					}
					return fallbackResult2;
				} catch (fallbackError) {
					console.error('Error executing search command:', fallbackError);
					// Both delegation and fallback failed - provide detailed error
					return formatErrorForAI(fallbackError);
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
				return formatErrorForAI(error);
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
	const { debug = false, outline = false, hashLines = false } = options;

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
				let extractFiles = null; // Track resolved file targets for content hashing

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
					extractFiles = parsedTargets.map(target => resolveTargetPath(target, effectiveCwd));

					// Auto-fix: if resolved paths don't exist, try allowedFolders subdirs
					// Handles when search returns relative paths (e.g., "gateway/file.go") and
					// model constructs wrong absolute paths (e.g., /workspace/gateway/file.go
					// instead of /workspace/tyk/gateway/file.go)
					if (options.allowedFolders && options.allowedFolders.length > 0) {
						const { join: pathJoin, sep: pathSep } = await import('path');
						extractFiles = extractFiles.map(target => {
							const { filePart, suffix } = splitTargetSuffix(target);
							if (existsSync(filePart)) return target;

							// Try resolving the relative tail against each allowedFolder
							const cwdPrefix = effectiveCwd.endsWith(pathSep) ? effectiveCwd : effectiveCwd + pathSep;
							const relativePart = filePart.startsWith(cwdPrefix)
								? filePart.slice(cwdPrefix.length)
								: null;

							if (relativePart) {
								for (const folder of options.allowedFolders) {
									const candidate = pathJoin(folder, relativePart);
									if (existsSync(candidate)) {
										if (debug) console.error(`[extract] Auto-fixed path: ${filePart} → ${candidate}`);
										return candidate + suffix;
									}
								}
							}

							// Try stripping workspace prefix and resolving against allowedFolders
							// e.g., /tmp/visor-workspaces/abc/gateway/file.go → try each folder + gateway/file.go
							for (const folder of options.allowedFolders) {
								const folderPrefix = folder.endsWith(pathSep) ? folder : folder + pathSep;
								const sepEscaped = pathSep === '\\' ? '\\\\' : pathSep;
								const wsParent = folderPrefix.replace(new RegExp('[^' + sepEscaped + ']+' + sepEscaped + '$'), '');
								if (filePart.startsWith(wsParent)) {
									const tail = filePart.slice(wsParent.length);
									const candidate = pathJoin(folderPrefix, tail);
									if (candidate !== filePart && existsSync(candidate)) {
										if (debug) console.error(`[extract] Auto-fixed path via workspace: ${filePart} → ${candidate}`);
										return candidate + suffix;
									}
								}
							}

							return target;
						});
					}

					// Apply format mapping for outline-xml to xml
					let effectiveFormat = format;
					if (outline && format === 'outline-xml') {
						effectiveFormat = 'xml';
					}

					// Set up extract options with files
					extractOptions = {
						files: extractFiles,
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

				// Track files and symbol content for staleness detection (post-extract)
				if (options.fileTracker && extractFiles && extractFiles.length > 0) {
					options.fileTracker.trackFilesFromExtract(extractFiles, effectiveCwd).catch(() => {});
				}

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

				if (hashLines && typeof results === 'string') {
					return annotateOutputWithHashes(results);
				}
				return results;
			} catch (error) {
				console.error('Error executing extract command:', error);
				return formatErrorForAI(error);
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
	const { debug = false, timeout = 300, cwd, allowedFolders, workspaceRoot, enableBash = false, bashConfig, architectureFileName, enableMcp = false, mcpConfig = null, mcpConfigPath = null, delegationManager = null } = options;

	return tool({
		name: 'delegate',
		description: delegateDescription,
		inputSchema: delegateSchema,
		execute: async ({ task, currentIteration, maxIterations, parentSessionId, path, provider, model, tracer, searchDelegate, parentAbortSignal }) => {
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
			// Delegation uses: path || workspaceRoot || cwd
			//
			// This is intentional because:
			// - Other tools operate within the parent's navigation context (cwd is correct)
			// - Subagents need a FRESH start from workspace root, not parent's navigation state
			// - Using parent's cwd would cause "path doubling" (Issue #348) where paths like
			//   /workspace/project/src/internal/build/src/internal/build/file.go get constructed
			//
			// The workspace root (computed as common prefix of allowedFolders) is the security
			// boundary and correct base for subagent operations. Parent navigation context
			// should not leak to subagents.
			//
			// NOTE: This priority (workspaceRoot > allowedFolders[0], excluding cwd) is INTENTIONALLY
			// different from other tools (bashTool uses workspaceRoot > cwd > allowedFolders[0]).
			// This prevents parent's navigation state from leaking to subagents.
			const effectiveWorkspaceRoot = workspaceRoot || (allowedFolders && allowedFolders[0]);
			const effectivePath = path || effectiveWorkspaceRoot || cwd;

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
				searchDelegate,
				enableMcp,
				mcpConfig,
				mcpConfigPath,
				delegationManager,  // Per-instance delegation limits
				parentAbortSignal
			});

			return result;
		}
	});
};

/**
 * Analyze All tool generator - intelligent 3-phase analysis using map-reduce
 *
 * @param {Object} [options] - Configuration options
 * @param {string} [options.sessionId] - Session ID for caching
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string} [options.cwd] - Working directory
 * @param {string[]} [options.allowedFolders] - Allowed folders
 * @param {string} [options.provider] - AI provider
 * @param {string} [options.model] - AI model
 * @param {Object} [options.tracer] - Telemetry tracer
 * @returns {Object} Configured analyze_all tool
 */
export const analyzeAllTool = (options = {}) => {
	const { sessionId, debug = false, delegationManager = null, workspaceRoot } = options;

	return tool({
		name: 'analyze_all',
		description: analyzeAllDescription,
		inputSchema: analyzeAllSchema,
		execute: async ({ question, path }) => {
			try {
				// Parse and resolve path if provided
				let searchPath = path || '.';
				if (path && options.cwd) {
					const resolvedPaths = parseAndResolvePaths(path, options.cwd);
					if (resolvedPaths.length > 0) {
						searchPath = resolvedPaths[0];
					}
				}

				if (debug) {
					console.error(`[analyze_all] Starting analysis`);
					console.error(`[analyze_all] Question: ${question}`);
					console.error(`[analyze_all] Path: ${searchPath}`);
				}

				// Use workspaceRoot (computed common prefix) for consistent path handling
				// Priority: workspaceRoot > cwd > allowedFolders[0] (consistent with bashTool)
				const effectiveWorkspaceRoot = workspaceRoot || options.cwd || (options.allowedFolders && options.allowedFolders[0]);

				const result = await analyzeAll({
					question,
					path: searchPath,
					sessionId,
					debug,
					cwd: options.cwd,
					workspaceRoot: effectiveWorkspaceRoot,
					allowedFolders: options.allowedFolders,
					provider: options.provider,
					model: options.model,
					tracer: options.tracer,
					delegationManager,  // Per-instance delegation limits
					parentAbortSignal: options.parentAbortSignal || null
				});

				return result;
			} catch (error) {
				console.error('Error executing analyze_all:', error);
				return formatErrorForAI(error);
			}
		}
	});
};
