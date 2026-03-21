/**
 * Tools for Vercel AI SDK
 * @module tools/vercel
 */

import { tool, generateText } from 'ai';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { symbols } from '../symbols.js';
import { delegate } from '../delegate.js';
import { analyzeAll } from './analyzeAll.js';
import { searchSchema, searchDelegateSchema, querySchema, extractSchema, symbolsSchema, delegateSchema, analyzeAllSchema, searchDescription, searchDelegateDescription, queryDescription, extractDescription, delegateDescription, analyzeAllDescription, parseTargets, parseAndResolvePaths, resolveTargetPath } from './common.js';
import { existsSync } from 'fs';
import { formatErrorForAI } from '../utils/error-types.js';
import { annotateOutputWithHashes } from './hashline.js';
import { createLanguageModel } from '../utils/provider.js';
import { truncateForSpan } from '../agent/simpleTelemetry.js';

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
		confidence: {
			type: 'string',
			enum: ['high', 'medium', 'low'],
			description: 'How confident you are that these locations answer the question.'
		},
		reason: {
			type: 'string',
			description: 'Brief explanation of confidence level — what was found, partially found, or not found.'
		},
		groups: {
			type: 'array',
			items: {
				type: 'object',
				properties: {
					reason: {
						type: 'string',
						description: 'Why these files are relevant — what aspect of the question they address (not how the code works).'
					},
					files: {
						type: 'array',
						items: { type: 'string' },
						description: 'File targets like "path/to/file.ext#Symbol" or "path/to/file.ext:10-20".'
					}
				},
				required: ['reason', 'files']
			},
			description: 'Groups of related files, each with a reason explaining why they matter.'
		},
		searches: {
			type: 'array',
			items: {
				type: 'object',
				properties: {
					query: { type: 'string', description: 'The search query used.' },
					path: { type: 'string', description: 'The path searched in.' },
					had_results: { type: 'boolean', description: 'Whether the search returned any results.' }
				},
				required: ['query', 'path', 'had_results']
			},
			description: 'All search queries executed during this session, with their paths and outcomes.'
		}
	},
	required: ['confidence', 'reason', 'groups', 'searches'],
	additionalProperties: false
};

/**
 * LLM-based semantic dedup for delegate queries.
 * Asks the same model to classify a new query against previous ones.
 * Returns: { action: 'allow'|'block'|'rewrite', rewritten?: string, reason: string }
 */
async function checkDelegateDedup(newQuery, previousQueries, model, debug) {
	if (!model || previousQueries.length === 0) {
		return { action: 'allow', reason: 'no previous queries' };
	}

	const previousList = previousQueries
		.map((q, i) => {
			let line = `${i + 1}. "${q.query}" (path: ${q.path}, found results: ${q.hadResults})`;
			if (q.reason) line += `\n   Outcome: ${q.reason}`;
			if (q.groups && q.groups.length > 0) {
				line += `\n   Found: ${q.groups.map(g => g.reason).join('; ')}`;
			}
			return line;
		})
		.join('\n');

	try {
		const result = await generateText({
			model,
			maxTokens: 150,
			temperature: 0,
			prompt: `You decide if a code search query is redundant given previous queries in the same session.

PREVIOUS QUERIES:
${previousList}

NEW QUERY: "${newQuery}"

Respond with exactly one line: ACTION|REASON
For rewrites: rewrite|REASON|REWRITTEN_QUERY

BLOCK when:
- Same concept, different phrasing: "find X" / "definition of X" / "where is X" / "X implementation" → all the same
- Synonym or narrower term of a previous query: "dedup" → "duplicate" → "unique" → all the same concept
- Single generic word that's just a synonym of a previous failed query
- Query is trying to brute-force the same concept with different keywords after previous failures

REWRITE when:
- Previous query was too narrow and failed, new query targets the same goal but could use a FUNDAMENTALLY different search strategy (e.g. searching for a caller instead of the function name, or searching the config/registration site instead of the implementation)
- Previous query found WRONG results (e.g. found "FallbackManager" when looking for "dedup logic") — rewrite to target the actual concept more precisely using implementation-level terms

ALLOW only when:
- The new query targets a COMPLETELY DIFFERENT feature, module, or subsystem — not just a different word for the same thing

Only BLOCK when you are CERTAIN the queries target the same concept. When uncertain, ALLOW — a missed dedup is cheaper than blocking a valid search.

Examples:
- Prev: "wrapToolWithEmitter" → New: "definition of wrapToolWithEmitter" → block|Same symbol
- Prev: "search dedup" (no results) → New: "dedup" → block|Synonym of failed query
- Prev: "dedup" (no results) → New: "duplicate" → block|Synonym of failed query
- Prev: "dedup" (no results) → New: "unique" → block|Synonym of failed query
- Prev: "auth middleware" → New: "rate limiting" → allow|Different subsystem
- Prev: "search dedup" (no results) → New: "previousSearches Map" → rewrite|Searching for implementation detail instead of concept|previousSearches OR searchKey`
		});

		const line = result.text.trim().split('\n')[0];
		const parts = line.split('|');
		const action = (parts[0] || '').toLowerCase().trim();

		if (action === 'block') {
			return { action: 'block', reason: parts[1]?.trim() || 'duplicate query' };
		} else if (action === 'rewrite' && parts[2]) {
			return { action: 'rewrite', reason: parts[1]?.trim() || 'refined query', rewritten: parts[2].trim() };
		}
		return { action: 'allow', reason: parts[1]?.trim() || 'new concept' };
	} catch (err) {
		if (debug) console.error('[DEDUP-LLM] Error:', err.message);
		return { action: 'allow', reason: 'dedup check failed, allowing' };
	}
}

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

/**
 * Parse the delegate sub-agent's raw response into a structured result.
 * Returns { confidence, groups } when possible, or builds a single-group
 * fallback from legacy { targets: [...] } or plain text.
 */
function parseDelegatedResponse(rawResponse) {
	if (!rawResponse || typeof rawResponse !== 'string') return null;
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
		// New format: { confidence, groups: [{ reason, files }], searches: [...] }
		if (Array.isArray(parsed.groups)) {
			return {
				confidence: parsed.confidence || 'medium',
				reason: parsed.reason || '',
				groups: parsed.groups.map(g => ({
					reason: g.reason || '',
					files: normalizeTargets(g.files || [])
				})).filter(g => g.files.length > 0),
				searches: Array.isArray(parsed.searches) ? parsed.searches : []
			};
		}
		// Legacy format: { targets: [...] }
		if (Array.isArray(parsed.targets)) {
			const files = normalizeTargets(parsed.targets);
			if (files.length > 0) {
				return { confidence: 'medium', reason: '', groups: [{ reason: 'Search results', files }], searches: [] };
			}
			// Empty targets array — explicitly return null (don't fall through to text fallback)
			return null;
		}
		// Plain array
		if (Array.isArray(parsed)) {
			const files = normalizeTargets(parsed);
			if (files.length > 0) {
				return { confidence: 'medium', reason: '', groups: [{ reason: 'Search results', files }], searches: [] };
			}
			return null;
		}
	}

	// Fallback: extract targets from plain text
	const files = normalizeTargets(fallbackTargetsFromText(trimmed));
	if (files.length > 0) {
		return { confidence: 'low', reason: '', groups: [{ reason: 'Search results', files }], searches: [] };
	}
	return null;
}

// Keep backward compat for any other callers
function parseDelegatedTargets(rawResponse) {
	const result = parseDelegatedResponse(rawResponse);
	if (!result) return [];
	return result.groups.flatMap(g => g.files);
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
	return `<role>
You are a code-location subagent. Your job is to find WHERE relevant code lives for the given question.
You are NOT answering the question — you are finding the code locations that would help answer it.
</role>

<task>
<question>${searchQuery}</question>
<search-path>${searchPath}</search-path>
<options language="${language || 'auto'}" allow_tests="${allowTests ? 'true' : 'false'}" />
</task>

<tools>
<tool name="search">
Find code matching keywords or patterns. Results are paginated — use nextPage=true when results are relevant to get more.
</tool>
<tool name="extract">
Read code to verify a file is actually relevant before including it.
</tool>
<tool name="listFiles">
Browse directory structure to discover where code might live.
</tool>
</tools>

<search-engine-behavior>
- Probe handles stemming, case-insensitive matching, and camelCase/snake_case splitting automatically.
- "allowed_ips" ALREADY matches "AllowedIPs", "allowedIps", etc. Do NOT try case/style variations.
- NEVER repeat the same search query — you will get the same results.
- If a search returns no results at workspace root, the term does not exist. Move on.
- If a search returns no results in a subfolder, try the workspace root or a different directory.
- Use exact=true for known symbol names. Use default for conceptual/exploratory queries.
- Combine related symbols with OR: "SymbolA" OR "SymbolB" finds files with either.
- Run INDEPENDENT searches in PARALLEL — do not wait between unrelated searches.
</search-engine-behavior>

<strategy>
1. Analyze the question — identify key concepts and brainstorm what a developer would NAME the relevant code.
2. Start your first search with the FULL search-path provided above. Do NOT narrow to a subdirectory on first try — the code may live anywhere in the tree.
3. Search for the main concept and synonyms in parallel.
4. Use extract to verify relevance — skim the code to confirm it ACTUALLY relates to the question.
5. Follow the trail: if you find a function, look for its callers, type definitions, and registered handlers.
6. Group your findings by WHY they are relevant (not by how you found them).
</strategy>

<relevance-filtering priority="critical">
- Only include files you have VERIFIED are relevant by reading them with extract.
- Do NOT include files just because they matched a keyword — confirm the match is meaningful.
- A file that mentions "session" in a comment is NOT relevant to "How do sessions work?" — look for the actual implementation.
- Fewer verified-relevant files are far more valuable than many unverified keyword matches.
- If a file is tangentially related but not core to the question, leave it out.
- If NO files are truly relevant, return EMPTY groups with confidence "low". An honest empty result is far better than a wrong result. Never fill groups with loosely related files just to have something.
</relevance-filtering>

<stop-conditions>
- Once you have found locations covering the main concept and related subsystems.
- If 2-3 different search approaches fail, stop and report what you have.
- Do NOT keep trying quote/syntax variations of the same failing keyword.
</stop-conditions>

<on-iteration-limit>
If you run out of tool iterations, you MUST still output your JSON response with whatever you found so far.
Set confidence to "low" if your search was incomplete.
Include ALL files you verified as relevant, even if coverage is partial.
The "searches" field helps the caller understand what was attempted.
</on-iteration-limit>

<output-rules>
- Return ONLY valid JSON matching the schema. No markdown, no explanation.
- ONLY include files you have verified are relevant. No noise.
- Group files by RELEVANCE to the question, not by search query.
- Use ABSOLUTE file paths. Prefer #Symbol for functions/classes; otherwise use line ranges.
- Deduplicate files across groups.
</output-rules>`;
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

	// Track previous non-paginated searches: key → { hadResults: boolean }
	const previousSearches = new Map();
	// Track per-key consecutive block counts (not global, to avoid cross-query pollution)
	const dupBlockCounts = new Map();
	// Track pagination counts per query to cap runaway pagination
	const paginationCounts = new Map();
	// Track consecutive no-result searches (circuit breaker)
	let consecutiveNoResults = 0;
	const MAX_CONSECUTIVE_NO_RESULTS = 4;
	// Track normalized query concepts for fuzzy dedup (catches quote/syntax variations)
	const failedConcepts = new Map(); // normalizedKey → count
	const MAX_PAGES_PER_QUERY = 3;

	// Track delegated searches at the PARENT level to prevent the pro model from
	// spawning redundant delegates for the same concept. Each delegate is expensive
	// (full flash agent session), so blocking repeats saves minutes.
	// LLM-based semantic dedup replaces deterministic normalization for delegates.
	const previousDelegations = []; // { query: string, path: string, hadResults: boolean }
	let cachedDedupModel = undefined; // lazily initialized

	/**
	 * Normalize a search query to detect syntax-level duplicates.
	 * Strips quotes, dots, underscores/hyphens, and lowercases.
	 * "ctxGetData", "ctx.GetData", "ctx_get_data" all → "ctxgetdata"
	 * Note: does NOT strip language keywords (func, type) — those change search
	 * semantics and are already handled as stopwords by the Rust search engine.
	 */
	function normalizeQueryConcept(query) {
		if (!query) return '';
		return query
			.replace(/^["']|["']$/g, '')      // strip outer quotes
			// Strip filler prefixes: "definition of X", "find X", "where is X", etc.
			.replace(/^(definition\s+of|implementation\s+of|usage\s+of|find|where\s+is|how\s+does|locate|show\s+me|get|look\s+for)\s+/i, '')
			.replace(/^["']|["']$/g, '')      // strip quotes again after prefix removal
			.replace(/\./g, '')                 // "ctx.GetData" → "ctxGetData"
			.replace(/[_\-\s]+/g, '')           // strip underscores/hyphens/spaces
			.toLowerCase()
			.trim();
	}

	return tool({
		name: 'search',
		description: searchDelegate
			? searchDelegateDescription
			: searchDescription,
		inputSchema: searchDelegate ? searchDelegateSchema : searchSchema,
		execute: async ({ query: searchQuery, path, allow_tests, exact, maxTokens: paramMaxTokens, language, session, nextPage, workingDirectory }) => {
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

			// Use workingDirectory (injected by _buildNativeTools at runtime) > cwd from config > fallback
			const effectiveSearchCwd = workingDirectory || options.cwd || '.';

			// Parse and resolve paths (supports comma-separated and relative paths)
			let searchPaths;
			if (path) {
				searchPaths = parseAndResolvePaths(path, effectiveSearchCwd);
			}

			// Default to cwd or '.' if no paths provided
			if (!searchPaths || searchPaths.length === 0) {
				searchPaths = [effectiveSearchCwd];
			}

			// Join paths with space for CLI (probe search supports multiple paths)
			const searchPath = searchPaths.join(' ');

			const searchOptions = {
				query: searchQuery,
				path: searchPath,
				cwd: effectiveSearchCwd, // Working directory for resolving relative paths
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
				// Include path in dedup key so same query across different repos is allowed (#520)
				const searchKey = `${searchPath}::${searchQuery}::${exact || false}::${language || ''}`;
				let circuitBreakerWarning = '';
				if (!nextPage) {
					if (previousSearches.has(searchKey)) {
						const blockCount = (dupBlockCounts.get(searchKey) || 0) + 1;
						dupBlockCounts.set(searchKey, blockCount);
						if (debug) {
							console.error(`[DEDUP] Blocked duplicate search (${blockCount}x): "${searchQuery}" (path: "${searchPath}")`);
						}
						if (blockCount >= 3) {
							return 'STOP. You have been blocked ' + blockCount + ' times for repeating the same search. You MUST provide your final answer NOW with whatever information you have. Do NOT call any more tools.';
						}
						const prev = previousSearches.get(searchKey);
						if (prev.hadResults) {
							return `DUPLICATE SEARCH BLOCKED (${blockCount}x). You already searched for "${searchQuery}" in this path and found results. Do NOT repeat. Use extract to examine the files you already found, try a COMPLETELY different keyword, or provide your final answer.`;
						}
						const exactHint = exact
							? 'You used exact=true. Try a broader search with exact=false, or use listFiles to browse the directory structure.'
							: 'Try exact=true if you need literal/punctuation matching (e.g. \'description: ""\'), or use listFiles to explore directories, or search for a broader/related term and filter manually.';
						return `DUPLICATE SEARCH BLOCKED (${blockCount}x). You already searched for "${searchQuery}" in this path and got NO results. This term does not appear in the codebase. Do NOT repeat or rephrase — try a FUNDAMENTALLY different approach: ${exactHint} If multiple approaches have failed, provide your final answer with what you know.`;
					}
					previousSearches.set(searchKey, { hadResults: false });
					paginationCounts.set(searchKey, 0);

					// Fuzzy concept dedup: catch quote/syntax variations of the same failed concept
					// e.g., "func ctxGetData", "ctxGetData", "ctx.GetData" all normalize to "ctxgetdata"
					const normalizedKey = `${searchPath}::${normalizeQueryConcept(searchQuery)}`;
					if (failedConcepts.has(normalizedKey) && failedConcepts.get(normalizedKey) >= 2) {
						const conceptCount = failedConcepts.get(normalizedKey) + 1;
						failedConcepts.set(normalizedKey, conceptCount);
						if (debug) {
							console.error(`[CONCEPT-DEDUP] Blocked variation of failed concept (${conceptCount}x): "${searchQuery}" normalized to "${normalizeQueryConcept(searchQuery)}"`);
						}
						const isSubfolder = path && path !== effectiveSearchCwd && path !== '.';
						const scopeHint = isSubfolder
							? `\n- Try searching from the workspace root (omit the path parameter) — the term may exist in a different directory`
							: `\n- The term does not exist in this codebase at any path`;
						return `CONCEPT ALREADY FAILED (${conceptCount} variations tried). You already searched for "${normalizeQueryConcept(searchQuery)}" with different quoting/syntax in this path and got NO results each time. Changing quotes, adding "func" prefix, or switching to method syntax will NOT change the results.\n\nChange your strategy:${scopeHint}\n- Use extract on a file you ALREADY found to read actual code and discover real function/type names\n- Use listFiles to browse directories and find what functions actually exist\n- Search for a BROADER concept (e.g., instead of "ctxGetData", try "context" or "middleware data access")\n- If you have enough information from prior searches, provide your final answer NOW`;
					}

					// Circuit breaker: too many consecutive no-result searches means the model
					// is stuck in a loop guessing names that don't exist.
					// Not permanent: allow the search through but prepend a strong warning.
					// If it succeeds, consecutiveNoResults resets to 0 (line ~598).
					// If it fails, the counter keeps climbing and subsequent attempts
					// get increasingly stern warnings.
					if (consecutiveNoResults >= MAX_CONSECUTIVE_NO_RESULTS) {
						if (debug) {
							console.error(`[CIRCUIT-BREAKER] ${consecutiveNoResults} consecutive no-result searches, warning: "${searchQuery}"`);
						}
						const isSubfolderCB = path && path !== effectiveSearchCwd && path !== '.';
						const cbScopeHint = isSubfolderCB
							? ` You have been searching in "${path}" — consider searching from the workspace root or a different directory.`
							: '';
						circuitBreakerWarning = `\n\n⚠️ CIRCUIT BREAKER: Your last ${consecutiveNoResults} searches ALL returned no results.${cbScopeHint} You MUST change your approach: use extract on files you already found, use listFiles to browse directories, or provide your final answer. Guessing names will not help.`;
					}
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
					// Track whether this search had results for better dedup messages
					if (typeof result === 'string' && result.includes('No results found')) {
						// Track consecutive no-results and failed concepts for circuit breaker
						consecutiveNoResults++;
						const normalizedKey = `${searchPath}::${normalizeQueryConcept(searchQuery)}`;
						failedConcepts.set(normalizedKey, (failedConcepts.get(normalizedKey) || 0) + 1);
						if (debug) {
							console.error(`[NO-RESULTS] consecutiveNoResults=${consecutiveNoResults}, concept "${normalizeQueryConcept(searchQuery)}" failed ${failedConcepts.get(normalizedKey)}x`);
						}
						// Append contextual hint for ticket/issue ID queries
						if (/^[A-Z]+-\d+$/.test(searchQuery.trim()) || /^[A-Z]+-\d+$/.test(searchQuery.replace(/"/g, '').trim())) {
							return result + '\n\n⚠️ Your query looks like a ticket/issue ID (e.g., JIRA-1234). Ticket IDs are rarely present in source code. Search for the technical concepts described in the ticket instead (e.g., function names, error messages, variable names).' + circuitBreakerWarning;
						}
						// Add a hint when approaching the circuit breaker threshold
						if (consecutiveNoResults >= MAX_CONSECUTIVE_NO_RESULTS - 1 && !circuitBreakerWarning) {
							const isSubfolderWarn = path && path !== effectiveSearchCwd && path !== '.';
							const warnScopeHint = isSubfolderWarn
								? ` You are searching in "${path}" — consider searching from the workspace root or a different directory.`
								: '';
							return result + `\n\n⚠️ WARNING: ${consecutiveNoResults} consecutive searches returned no results.${warnScopeHint} Before your next action: use extract on a file you already found to read actual code, or use listFiles to discover what functions really exist. One more failed search will trigger the circuit breaker.`;
						}
					} else if (typeof result === 'string') {
						// Successful search — reset consecutive counter
						consecutiveNoResults = 0;
						const entry = previousSearches.get(searchKey);
						if (entry) entry.hadResults = true;
					}
					// Track files found in search results for staleness detection
					if (options.fileTracker && typeof result === 'string') {
						options.fileTracker.trackFilesFromOutput(result, effectiveSearchCwd).catch(() => {});
					}
					return typeof result === 'string' ? result + circuitBreakerWarning : result;
				} catch (error) {
					console.error('Error executing search command:', error);
					const formatted = formatErrorForAI(error);
					if (error.category === 'path_error' || error.message?.includes('does not exist')) {
						return formatted + '\n\nThe path does not exist. Use the listFiles tool to verify the correct directory structure before retrying. If the workspace itself is gone, output your final answer with whatever information you have.';
					}
					return formatted;
				}
			}

			// ── Delegate-level semantic dedup ────────────────────────────
				// Each delegate is a full flash agent session (minutes, not seconds).
				// Use LLM to detect semantic duplicates and suggest rewrites.
				// Compare against ALL previous delegations (not filtered by path) because
				// the parent model often narrows the path while asking the same concept
				// (e.g., "dedup" at /src → "deduplicate" at /src/search.js).
				const delegatePath = searchPath || '';

				let effectiveQuery = searchQuery;

				if (previousDelegations.length > 0) {
					// Lazily create the dedup model (same provider/model as delegate)
					if (cachedDedupModel === undefined) {
						const dedupProvider = options.searchDelegateProvider || process.env.PROBE_SEARCH_DELEGATE_PROVIDER || options.provider || process.env.FORCE_PROVIDER || null;
						const dedupModelName = options.searchDelegateModel || process.env.PROBE_SEARCH_DELEGATE_MODEL || options.model || process.env.MODEL_NAME || null;
						if (debug) {
							console.error(`[DEDUP-LLM] Creating model: provider=${dedupProvider}, model=${dedupModelName}`);
						}
						cachedDedupModel = await createLanguageModel(dedupProvider, dedupModelName);
						if (debug) {
							console.error(`[DEDUP-LLM] Model created: ${cachedDedupModel ? 'success' : 'null'}`);
						}
					}

					const dedupSpanAttrs = {
						'dedup.query': searchQuery,
						'dedup.previous_count': String(previousDelegations.length),
						'dedup.previous_queries': previousDelegations.map(d => d.query).join(' | '),
					};

					const dedup = options.tracer?.withSpan
						? await options.tracer.withSpan('search.delegate.dedup', async () => {
							return await checkDelegateDedup(searchQuery, previousDelegations, cachedDedupModel, debug);
						}, dedupSpanAttrs, (span, result) => {
							span.setAttributes({
								'dedup.action': result.action,
								'dedup.reason': result.reason || '',
								'dedup.rewritten': result.rewritten || '',
							});
						})
						: await checkDelegateDedup(searchQuery, previousDelegations, cachedDedupModel, debug);

					if (debug) {
						console.error(`[DEDUP-LLM] Query: "${searchQuery}" → ${dedup.action}: ${dedup.reason}${dedup.rewritten ? ` → "${dedup.rewritten}"` : ''}`);
					}

					if (dedup.action === 'block') {
						const prevQueries = previousDelegations.map(d => `"${d.query}"`).join(', ');
						return `DELEGATE BLOCKED: "${searchQuery}" is semantically duplicate of previous delegation(s) [${prevQueries}]. ${dedup.reason}\n\nDo NOT re-delegate the same concept. Use extract() on files already found, or synthesize your answer from existing results.`;
					}

					if (dedup.action === 'rewrite' && dedup.rewritten) {
						effectiveQuery = dedup.rewritten;
						if (debug) {
							console.error(`[DEDUP-LLM] Rewritten query: "${searchQuery}" → "${effectiveQuery}"`);
						}
					}
				}

				// Record this delegation
				const delegationRecord = { query: effectiveQuery, path: delegatePath, hadResults: false };
				previousDelegations.push(delegationRecord);

			try {
				if (debug) {
					console.error(`Delegating search with query: "${effectiveQuery}", path: "${searchPath}"${effectiveQuery !== searchQuery ? ` (rewritten from: "${searchQuery}")` : ''}`);
				}

				const delegateTask = buildSearchDelegateTask({
					searchQuery: effectiveQuery,
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
					allowEdit: options.allowEdit || false,
					architectureFileName: options.architectureFileName || null,
					promptType: 'code-searcher',
					allowedTools: ['search', 'extract', 'listFiles'],
					searchDelegate: false,
					schema: CODE_SEARCH_SCHEMA,
					parentAbortSignal: options.parentAbortSignal || null
				});

				const delegateResult = options.tracer?.withSpan
					? await options.tracer.withSpan('search.delegate', runDelegation, {
						'search.query': searchQuery,
						'search.path': searchPath,
						...(effectiveQuery !== searchQuery ? { 'search.query.rewritten': effectiveQuery } : {})
					}, (span, result) => {
						const text = typeof result === 'string' ? result : JSON.stringify(result) || '';
						if (debug) console.error(`[search-delegate] onResult: type=${typeof result}, length=${text.length}`);
						span.setAttributes({
							'search.delegate.output': truncateForSpan(text),
							'search.delegate.output_length': String(text.length)
						});
					})
					: await runDelegation();

				const structured = parseDelegatedResponse(delegateResult);
				// Update delegation tracking with outcome (feeds into LLM dedup context)
				if (delegationRecord && structured) {
					delegationRecord.hadResults = structured.groups.length > 0;
					delegationRecord.reason = structured.reason || '';
					delegationRecord.groups = structured.groups.map(g => ({ reason: g.reason }));
				}
				if (!structured || structured.groups.length === 0) {
					// If the delegate explicitly concluded nothing was found (low confidence + reason),
					// return that verdict instead of falling back to raw search which would
					// return tangentially related results and mislead the parent agent.
					if (structured && structured.confidence === 'low' && structured.reason) {
						if (debug) {
							console.error(`Delegated search explicitly found nothing: ${structured.reason}`);
						}
						return `NOT FOUND: The search delegate thoroughly searched for "${searchQuery}" and concluded: ${structured.reason}\n\nDo NOT search for analogies or loosely related concepts. If the feature does not exist in the codebase, say so in your final answer.`;
					}
					if (debug) {
						console.error('Delegated search returned no results; falling back to raw search');
					}
					const fallbackResult = maybeAnnotate(await runRawSearch());
					if (options.fileTracker && typeof fallbackResult === 'string') {
						options.fileTracker.trackFilesFromOutput(fallbackResult, effectiveSearchCwd).catch(() => {});
					}
					return fallbackResult;
				}

				// Resolve and validate file paths in each group
				const delegateBase = options.allowedFolders?.[0] || options.cwd || '.';
				const resolutionBase = searchPaths[0] || options.cwd || '.';
				const wsPrefix = resolutionBase.endsWith('/') ? resolutionBase : resolutionBase + '/';

				for (const group of structured.groups) {
					group.files = group.files
						.map(target => resolveTargetPath(target, delegateBase))
						.map(target => {
							const { filePart, suffix } = splitTargetSuffix(target);

							// 1. Path exists as-is
							if (existsSync(filePart)) return target;

							// 2. Fix doubled directory segments: /ws/proj/proj/src → /ws/proj/src
							const parts = filePart.split('/').filter(Boolean);
							for (let i = 0; i < parts.length - 1; i++) {
								if (parts[i] === parts[i + 1]) {
									const candidate = '/' + [...parts.slice(0, i), ...parts.slice(i + 1)].join('/');
									if (existsSync(candidate)) {
										if (debug) console.error(`[search-delegate] Fixed doubled path: ${filePart} → ${candidate}`);
										return candidate + suffix;
									}
								}
							}

							// 3. Try alternative bases
							for (const altBase of [resolutionBase, options.cwd].filter(Boolean)) {
								if (altBase === delegateBase) continue;
								const altResolved = resolveTargetPath(target, altBase);
								const { filePart: altFile } = splitTargetSuffix(altResolved);
								if (existsSync(altFile)) {
									if (debug) console.error(`[search-delegate] Resolved with alt base: ${filePart} → ${altFile}`);
									return altResolved;
								}
							}

							if (debug) console.error(`[search-delegate] Warning: target may not exist: ${filePart}`);
							return target;
						})
						// Strip workspace prefix to make paths relative
						.map(target => target.split(wsPrefix).join(''));
				}

				// Return structured JSON for the parent AI to decide what to extract
				return JSON.stringify(structured, null, 2);
			} catch (error) {
				console.error('Delegated search failed, falling back to raw search:', error);
				try {
					const fallbackResult2 = maybeAnnotate(await runRawSearch());
					if (options.fileTracker && typeof fallbackResult2 === 'string') {
						options.fileTracker.trackFilesFromOutput(fallbackResult2, effectiveSearchCwd).catch(() => {});
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
		execute: async ({ targets, input_content, line, end_line, allow_tests, context_lines, format, workingDirectory }) => {
			try {
				// Use workingDirectory (injected by _buildNativeTools at runtime) > cwd from config > fallback
				const effectiveCwd = workingDirectory || options.cwd || '.';

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
	const { debug = false, timeout = 300, cwd, allowedFolders, workspaceRoot, enableBash = false, bashConfig, allowEdit = false, architectureFileName, enableMcp = false, mcpConfig = null, mcpConfigPath = null, promptType: parentPromptType, customPrompt: parentCustomPrompt = null, completionPrompt: parentCompletionPrompt = null, delegationManager = null,
		// Timeout settings inherited from parent agent
		timeoutBehavior, maxOperationTimeout, requestTimeout, gracefulTimeoutBonusSteps,
		negotiatedTimeoutBudget, negotiatedTimeoutMaxRequests, negotiatedTimeoutMaxPerRequest,
		parentOperationStartTime, onSubagentCreated, onSubagentCompleted } = options;

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
			// Cap delegate timeout to remaining parent budget (with 10% headroom)
			let effectiveTimeout = timeout;
			if (parentOperationStartTime && maxOperationTimeout) {
				const elapsed = Date.now() - parentOperationStartTime;
				const remaining = maxOperationTimeout - elapsed;
				const budgetCap = Math.max(30, Math.floor(remaining * 0.9 / 1000)); // seconds, min 30s
				if (budgetCap < effectiveTimeout) {
					effectiveTimeout = budgetCap;
					if (debug) {
						console.error(`[DELEGATE] Capping timeout from ${timeout}s to ${effectiveTimeout}s (remaining parent budget: ${Math.floor(remaining/1000)}s)`);
					}
					if (tracer) {
						tracer.addEvent('delegation.budget_capped', {
							'delegation.original_timeout_s': timeout,
							'delegation.effective_timeout_s': effectiveTimeout,
							'delegation.parent_elapsed_ms': elapsed,
							'delegation.parent_remaining_ms': remaining,
							'delegation.parent_session_id': parentSessionId,
						});
					}
				}
			}

			const result = await delegate({
				task,
				timeout: effectiveTimeout,
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
				allowEdit,
				bashConfig,
				architectureFileName,
				promptType: parentPromptType,  // Inherit parent's prompt type
				customPrompt: parentCustomPrompt,  // Inherit parent's custom system prompt
				completionPrompt: parentCompletionPrompt,  // Inherit parent's completion prompt
				searchDelegate,
				enableMcp,
				mcpConfig,
				mcpConfigPath,
				delegationManager,  // Per-instance delegation limits
				parentAbortSignal,
				// Inherit timeout settings for subagent
				timeoutBehavior,
				requestTimeout,
				gracefulTimeoutBonusSteps,
				// Subagent lifecycle callbacks for graceful stop coordination
				onSubagentCreated,
				onSubagentCompleted,
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

export const symbolsTool = (options = {}) => {
	return tool({
		name: 'symbols',
		description: 'List all symbols (functions, classes, structs, constants, etc.) in a file. Returns a hierarchical tree with line numbers — like a table of contents for code files.',
		inputSchema: symbolsSchema,
		execute: async ({ file }) => {
			try {
				let filePath = file;
				if (options.cwd) {
					const resolvedPaths = parseAndResolvePaths(file, options.cwd);
					if (resolvedPaths.length > 0) {
						filePath = resolvedPaths[0];
					}
				}

				const result = await symbols({
					files: [filePath],
					cwd: options.cwd,
					binaryOptions: options.binaryOptions
				});

				// Return formatted JSON
				if (result && result.length > 0) {
					return JSON.stringify(result[0], null, 2);
				}
				return 'No symbols found in file.';
			} catch (error) {
				console.error('Error executing symbols:', error);
				return formatErrorForAI(error);
			}
		}
	});
};
