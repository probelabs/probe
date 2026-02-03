/**
 * Intelligent bulk data analysis tool using agentic planning + map-reduce pattern
 *
 * Three-phase approach:
 * 1. PLANNING: Analyze the question, explore data, determine optimal strategy
 * 2. PROCESSING: Map-reduce with the determined strategy
 * 3. SYNTHESIS: Comprehensive final answer with evidence
 *
 * @module tools/analyzeAll
 */

import { search } from '../search.js';
import { delegate } from '../delegate.js';

// Default chunk size in tokens (should fit comfortably in LLM context)
const DEFAULT_CHUNK_SIZE_TOKENS = 8000;
// Maximum parallel workers for map phase
const MAX_PARALLEL_WORKERS = 3;
// Maximum chunks to process (safety limit)
const MAX_CHUNKS = 50;
// Rough estimate: 1 token ≈ 4 characters
const CHARS_PER_TOKEN = 4;

/**
 * Estimate token count from string length
 * @param {string} text - Text to estimate tokens for
 * @returns {number} Estimated token count
 */
function estimateTokens(text) {
	if (!text) return 0;
	return Math.ceil(text.length / CHARS_PER_TOKEN);
}

/**
 * Strip <result> tags from AI response
 * @param {string} text - Text that may contain <result> tags
 * @returns {string} Text with tags removed
 */
function stripResultTags(text) {
	if (!text) return text;
	return text
		.replace(/^<result>\s*/i, '')
		.replace(/\s*<\/result>$/i, '')
		.trim();
}

/**
 * Parse the planning phase result to extract strategy
 * @param {string} planningResult - Raw planning result from AI
 * @returns {Object} Parsed strategy with query, aggregation, extractionPrompt
 */
function parsePlanningResult(planningResult) {
	const result = {
		searchQuery: null,
		aggregation: 'summarize',
		extractionPrompt: null,
		reasoning: null
	};

	// Extract SEARCH_QUERY
	const queryMatch = planningResult.match(/SEARCH_QUERY:\s*(.+?)(?=\n[A-Z_]+:|$)/s);
	if (queryMatch) {
		result.searchQuery = queryMatch[1].trim();
	}

	// Extract AGGREGATION
	const aggMatch = planningResult.match(/AGGREGATION:\s*(summarize|list_unique|count|group_by)/i);
	if (aggMatch) {
		result.aggregation = aggMatch[1].toLowerCase();
	}

	// Extract EXTRACTION_PROMPT
	const extractMatch = planningResult.match(/EXTRACTION_PROMPT:\s*(.+?)(?=\n[A-Z_]+:|$)/s);
	if (extractMatch) {
		result.extractionPrompt = extractMatch[1].trim();
	}

	// Extract REASONING (optional)
	const reasoningMatch = planningResult.match(/REASONING:\s*(.+?)$/s);
	if (reasoningMatch) {
		result.reasoning = reasoningMatch[1].trim();
	}

	return result;
}

/**
 * Split search results into chunks that fit within token limits
 * @param {string} searchResults - Raw search results string
 * @param {number} chunkSizeTokens - Maximum tokens per chunk
 * @returns {Array<{id: number, total: number, content: string, estimatedTokens: number}>}
 */
function chunkResults(searchResults, chunkSizeTokens) {
	const chunks = [];
	const chunkSizeChars = chunkSizeTokens * CHARS_PER_TOKEN;

	// Split by file blocks (each file block starts with ```)
	// This ensures we don't split in the middle of a code block
	const fileBlocks = searchResults.split(/(?=^```)/m);

	let currentChunk = '';
	let currentTokens = 0;

	for (const block of fileBlocks) {
		const blockTokens = estimateTokens(block);

		// If a single block is larger than chunk size, we need to include it anyway
		// but in its own chunk
		if (blockTokens > chunkSizeTokens && currentChunk.length > 0) {
			// Save current chunk first
			chunks.push({
				id: chunks.length + 1,
				total: 0,
				content: currentChunk.trim(),
				estimatedTokens: currentTokens
			});
			currentChunk = '';
			currentTokens = 0;
		}

		// Check if adding this block would exceed chunk size
		if (currentTokens + blockTokens > chunkSizeTokens && currentChunk.length > 0) {
			// Save current chunk
			chunks.push({
				id: chunks.length + 1,
				total: 0,
				content: currentChunk.trim(),
				estimatedTokens: currentTokens
			});
			currentChunk = '';
			currentTokens = 0;
		}

		// Add block to current chunk
		currentChunk += block;
		currentTokens += blockTokens;
	}

	// Don't forget the last chunk
	if (currentChunk.trim().length > 0) {
		chunks.push({
			id: chunks.length + 1,
			total: 0,
			content: currentChunk.trim(),
			estimatedTokens: currentTokens
		});
	}

	// Update total count in all chunks
	const totalChunks = chunks.length;
	for (const chunk of chunks) {
		chunk.total = totalChunks;
	}

	return chunks;
}

/**
 * Process a single chunk using delegate
 * @param {Object} chunk - Chunk to process
 * @param {string} extractionPrompt - What to extract from the chunk
 * @param {Object} options - Delegate options
 * @returns {Promise<{chunk: Object, result: string}>}
 */
async function processChunk(chunk, extractionPrompt, options) {
	const task = `You are analyzing search results (chunk ${chunk.id} of ${chunk.total}).

Your task: ${extractionPrompt}

Search Results:
${chunk.content}

Instructions:
- Extract ALL relevant information matching the analysis task
- Be specific and include actual names, values, patterns found
- Format as a structured list if multiple items found
- If nothing relevant is found in this chunk, respond with "No relevant items found in this chunk."
- Do NOT summarize the code - extract the specific information requested
- IMPORTANT: When completing, always use the FULL format: <attempt_completion><result>YOUR ANSWER HERE</result></attempt_completion>
- Do NOT use the shorthand <attempt_complete></attempt_complete> format`;

	try {
		const result = await delegate({
			task,
			debug: options.debug,
			parentSessionId: options.sessionId,
			path: options.path,
			allowedFolders: options.allowedFolders,
			provider: options.provider,
			model: options.model,
			tracer: options.tracer,
			enableBash: false,
			promptType: 'code-researcher',
			allowedTools: ['extract'],
			maxIterations: 5,
			timeout: 120
		});

		return { chunk, result };
	} catch (error) {
		return { chunk, result: `Error processing chunk ${chunk.id}: ${error.message}` };
	}
}

/**
 * Process chunks in parallel with concurrency limit
 * @param {Array} chunks - Chunks to process
 * @param {string} extractionPrompt - What to extract
 * @param {number} maxWorkers - Maximum concurrent workers
 * @param {Object} options - Options to pass to processChunk
 * @returns {Promise<Array<{chunk: Object, result: string}>>}
 */
async function processChunksParallel(chunks, extractionPrompt, maxWorkers, options) {
	const results = [];
	const queue = [...chunks];
	const active = new Set();

	while (queue.length > 0 || active.size > 0) {
		while (active.size < maxWorkers && queue.length > 0) {
			const chunk = queue.shift();

			const promise = processChunk(chunk, extractionPrompt, options).then(result => {
				active.delete(promise);
				return result;
			});

			active.add(promise);

			if (options.debug) {
				console.error(`[analyze_all] Started processing chunk ${chunk.id}/${chunk.total}`);
			}
		}

		if (active.size > 0) {
			const result = await Promise.race(active);
			results.push(result);

			if (options.debug) {
				console.error(`[analyze_all] Completed chunk ${result.chunk.id}/${result.chunk.total}`);
			}
		}
	}

	results.sort((a, b) => a.chunk.id - b.chunk.id);
	return results;
}

/**
 * Aggregate results from all chunks based on aggregation strategy
 * @param {Array<{chunk: Object, result: string}>} chunkResults - Results from all chunks
 * @param {string} aggregation - Aggregation strategy
 * @param {string} extractionPrompt - Original extraction prompt for context
 * @param {Object} options - Delegate options
 * @returns {Promise<string>}
 */
async function aggregateResults(chunkResults, aggregation, extractionPrompt, options) {
	const meaningfulResults = chunkResults.filter(r =>
		r.result &&
		!r.result.toLowerCase().includes('no relevant items found') &&
		r.result.trim().length > 0
	);

	if (meaningfulResults.length === 0) {
		return 'No relevant information found across all chunks.';
	}

	if (meaningfulResults.length === 1) {
		return stripResultTags(meaningfulResults[0].result);
	}

	const chunkSummaries = meaningfulResults
		.map(r => `--- Chunk ${r.chunk.id} ---\n${stripResultTags(r.result)}`)
		.join('\n\n');

	const completionNote = `\n\nIMPORTANT: When completing, always use the FULL format: <attempt_completion><result>YOUR ANSWER HERE</result></attempt_completion>`;

	const aggregationPrompts = {
		summarize: `Synthesize these analyses into a comprehensive summary. Combine related findings, remove redundancy, and present a coherent overview.

Original task: ${extractionPrompt}

Chunk analyses:
${chunkSummaries}

Provide a unified summary that captures all key findings.${completionNote}`,

		list_unique: `Combine these lists and remove duplicates. Create a single deduplicated list of all unique items found.

Original task: ${extractionPrompt}

Chunk analyses:
${chunkSummaries}

Return a deduplicated, organized list of all unique items. Group related items if helpful.${completionNote}`,

		count: `Count and aggregate the findings from these analyses. Provide total counts and breakdowns.

Original task: ${extractionPrompt}

Chunk analyses:
${chunkSummaries}

Provide accurate counts and a summary of all occurrences found.${completionNote}`,

		group_by: `Group and categorize all items from these analyses. Organize findings into logical categories.

Original task: ${extractionPrompt}

Chunk analyses:
${chunkSummaries}

Organize all findings into clear categories with items listed under each.${completionNote}`
	};

	const aggregationTask = aggregationPrompts[aggregation] || aggregationPrompts.summarize;

	try {
		const result = await delegate({
			task: aggregationTask,
			debug: options.debug,
			parentSessionId: options.sessionId,
			path: options.path,
			allowedFolders: options.allowedFolders,
			provider: options.provider,
			model: options.model,
			tracer: options.tracer,
			enableBash: false,
			promptType: 'code-researcher',
			allowedTools: [],
			maxIterations: 5,
			timeout: 120
		});

		return result;
	} catch (error) {
		return `Aggregation failed (${error.message}). Raw results:\n\n${chunkSummaries}`;
	}
}

/**
 * PHASE 1: Planning - Analyze the question and determine optimal strategy
 * Uses a full agent that can explore the repository to understand its structure
 * before creating the search plan.
 * @param {string} question - The user's free-form question
 * @param {string} path - Path to search in
 * @param {Object} options - Delegate options
 * @returns {Promise<Object>} Strategy object with searchQuery, aggregation, extractionPrompt
 */
async function planAnalysis(question, path, options) {
	if (options.debug) {
		console.error(`[analyze_all] Phase 1: Planning analysis strategy (with exploration)...`);
	}

	const planningTask = `You are planning a bulk data analysis. Your goal is to find the BEST search strategy through exploration and experimentation.

QUESTION TO ANSWER: "${question}"
SEARCH SCOPE: ${path}

YOUR TASK: Explore, experiment, then plan.

STEP 1: EXPLORE THE REPOSITORY STRUCTURE
Use listFiles to understand:
- What types of files exist? (code, markdown, configs, etc.)
- What are the naming patterns? (e.g., "Playbook", "Debrief", "spec", "test")
- What directories and subdirectories exist?

STEP 2: TEST DIFFERENT SEARCH QUERIES
Run several experimental searches to find what works:
- Try different keyword combinations related to the question
- Test queries based on file naming patterns you discovered
- Check which queries return relevant results vs empty results
- Iterate until you find queries that actually return data

For example, if looking for customer data:
- First try: search for "customer" - see what comes back
- If empty, try: search for "playbook" or "debrief" (common doc names)
- Check the actual content to understand terminology used

STEP 3: CREATE THE FINAL PLAN
Based on your experiments, output the BEST search strategy.

Use attempt_completion with this EXACT format:

SEARCH_QUERY: <the query that WORKED in your experiments - use OR for multiple terms>
AGGREGATION: <summarize | list_unique | count | group_by>
EXTRACTION_PROMPT: <what to extract from each search result>
REASONING: <what you discovered, what queries you tested, why the final query works>

CRITICAL: Do NOT guess keywords. Actually run searches and see what returns results!`;

	try {
		// Planning phase - full agent with exploration capabilities
		const result = await delegate({
			task: planningTask,
			debug: options.debug,
			parentSessionId: options.sessionId,
			path: path,
			allowedFolders: [path],
			provider: options.provider,
			model: options.model,
			tracer: options.tracer,
			enableBash: false,
			promptType: 'code-researcher',
			// Full tool access for exploration and experimentation
			maxIterations: 15
			// timeout removed - inherit default from delegate (300s)
		});

		const plan = parsePlanningResult(stripResultTags(result));

		if (options.debug) {
			console.error(`[analyze_all] Planning complete:`);
			console.error(`[analyze_all]   Search Query: ${plan.searchQuery}`);
			console.error(`[analyze_all]   Aggregation: ${plan.aggregation}`);
			console.error(`[analyze_all]   Extraction: ${plan.extractionPrompt?.substring(0, 100)}...`);
		}

		return plan;
	} catch (error) {
		throw new Error(`Planning phase failed: ${error.message}`);
	}
}

/**
 * PHASE 3: Synthesis - Create comprehensive final answer
 * @param {string} question - Original question
 * @param {string} aggregatedData - Results from map-reduce phase
 * @param {Object} plan - The analysis plan used
 * @param {Object} options - Delegate options
 * @returns {Promise<string>} Final comprehensive answer
 */
async function synthesizeAnswer(question, aggregatedData, plan, options) {
	if (options.debug) {
		console.error(`[analyze_all] Phase 3: Synthesizing final answer...`);
	}

	const synthesisTask = `You analyzed a codebase to answer this question:

"${question}"

Analysis Strategy Used:
- Search Query: ${plan.searchQuery}
- Aggregation Method: ${plan.aggregation}
- Extraction Focus: ${plan.extractionPrompt}

Aggregated Analysis Results:
${aggregatedData}

Now provide a COMPREHENSIVE, DETAILED answer to the original question.

Your answer should:
1. **Directly answer the question** with a clear summary at the top
2. **Provide specific evidence** - include actual names, values, file locations where relevant
3. **Organize the information** logically (use categories, lists, or sections as appropriate)
4. **Note completeness** - mention if the analysis covered all relevant data or if there might be gaps
5. **Be thorough** - this is the final answer the user will see, make it complete and useful

Format your response as a well-structured document that fully answers: "${question}"

IMPORTANT: When completing, use the FULL format: <attempt_completion><result>YOUR ANSWER HERE</result></attempt_completion>`;

	try {
		const result = await delegate({
			task: synthesisTask,
			debug: options.debug,
			parentSessionId: options.sessionId,
			path: options.path,
			allowedFolders: options.allowedFolders,
			provider: options.provider,
			model: options.model,
			tracer: options.tracer,
			enableBash: false,
			promptType: 'code-researcher',
			allowedTools: [],
			maxIterations: 5
			// timeout removed - inherit default from delegate (300s)
		});

		return stripResultTags(result);
	} catch (error) {
		// If synthesis fails, return the aggregated data as fallback
		return `Analysis Results for: "${question}"\n\n${aggregatedData}`;
	}
}

/**
 * Analyze all data matching a question using intelligent 3-phase approach:
 * 1. PLANNING: Analyze question, determine optimal search and aggregation strategy
 * 2. PROCESSING: Map-reduce with parallel chunk processing
 * 3. SYNTHESIS: Comprehensive final answer with evidence
 *
 * @param {Object} options - Analysis options
 * @param {string} options.question - Free-form question to answer (e.g., "What features are customers using?")
 * @param {string} [options.path='.'] - Directory to search in
 * @param {string} [options.sessionId] - Session ID for caching
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string} [options.cwd] - Working directory
 * @param {string[]} [options.allowedFolders] - Allowed folders
 * @param {string} [options.provider] - AI provider
 * @param {string} [options.model] - AI model
 * @param {Object} [options.tracer] - Telemetry tracer
 * @param {number} [options.chunkSizeTokens] - Custom chunk size (default: 8000)
 * @param {number} [options.maxChunks] - Maximum chunks to process (default: 50)
 * @returns {Promise<string>} Comprehensive answer to the question
 */
export async function analyzeAll(options) {
	const {
		question,
		path = '.',
		sessionId,
		debug = false,
		cwd,
		allowedFolders,
		provider,
		model,
		tracer,
		chunkSizeTokens = DEFAULT_CHUNK_SIZE_TOKENS,
		maxChunks = MAX_CHUNKS
	} = options;

	if (!question) {
		throw new Error('The "question" parameter is required.');
	}

	const delegateOptions = {
		debug,
		sessionId,
		path: allowedFolders?.[0] || cwd || path,
		allowedFolders,
		provider,
		model,
		tracer
	};

	// ============================================================
	// PHASE 1: Planning
	// ============================================================
	if (debug) {
		console.error(`[analyze_all] Starting analysis`);
		console.error(`[analyze_all] Question: ${question}`);
		console.error(`[analyze_all] Path: ${path}`);
	}

	const plan = await planAnalysis(question, path, delegateOptions);

	if (!plan.searchQuery) {
		throw new Error('Planning phase failed to determine a search query.');
	}
	if (!plan.extractionPrompt) {
		throw new Error('Planning phase failed to determine an extraction prompt.');
	}

	// ============================================================
	// PHASE 2: Processing (Map-Reduce)
	// ============================================================
	if (debug) {
		console.error(`[analyze_all] Phase 2: Processing data with map-reduce...`);
	}

	// Get ALL search results (no token limit)
	const searchResults = await search({
		query: plan.searchQuery,
		path,
		cwd,
		maxTokens: null,
		allowTests: true,
		session: sessionId
	});

	if (!searchResults || searchResults.trim().length === 0) {
		return `No data found matching the analysis plan for: "${question}"\n\nSearch query used: ${plan.searchQuery}\n\nTry rephrasing your question or broadening the scope.`;
	}

	const totalTokens = estimateTokens(searchResults);
	if (debug) {
		console.error(`[analyze_all] Total search results: ~${totalTokens} tokens`);
	}

	let aggregatedData;

	// If results fit in a single chunk, process directly
	if (totalTokens <= chunkSizeTokens) {
		if (debug) {
			console.error(`[analyze_all] Results fit in single chunk, processing directly`);
		}

		const singleChunk = {
			id: 1,
			total: 1,
			content: searchResults,
			estimatedTokens: totalTokens
		};

		const result = await processChunk(singleChunk, plan.extractionPrompt, delegateOptions);
		aggregatedData = stripResultTags(result.result);
	} else {
		// Chunk and process in parallel
		const chunks = chunkResults(searchResults, chunkSizeTokens);

		if (debug) {
			console.error(`[analyze_all] Split into ${chunks.length} chunks`);
		}

		if (chunks.length > maxChunks) {
			console.error(`[analyze_all] Warning: Truncating from ${chunks.length} to ${maxChunks} chunks`);
			chunks.length = maxChunks;
			for (const chunk of chunks) {
				chunk.total = maxChunks;
			}
		}

		const chunkResultsProcessed = await processChunksParallel(
			chunks,
			plan.extractionPrompt,
			MAX_PARALLEL_WORKERS,
			delegateOptions
		);

		if (debug) {
			console.error(`[analyze_all] All ${chunks.length} chunks processed, starting aggregation`);
		}

		aggregatedData = await aggregateResults(
			chunkResultsProcessed,
			plan.aggregation,
			plan.extractionPrompt,
			delegateOptions
		);
		aggregatedData = stripResultTags(aggregatedData);
	}

	// ============================================================
	// PHASE 3: Synthesis
	// ============================================================
	const finalAnswer = await synthesizeAnswer(question, aggregatedData, plan, delegateOptions);

	if (debug) {
		console.error(`[analyze_all] Analysis complete`);
	}

	return finalAnswer;
}
