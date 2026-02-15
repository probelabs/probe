#!/usr/bin/env node
/**
 * Manual test script for the DSL runtime with real tools.
 *
 * Usage:
 *   node npm/src/agent/dsl/manual-test.mjs
 *
 * Requires: GOOGLE_API_KEY or GOOGLE_GENERATIVE_AI_API_KEY in .env or env
 */

import { createDSLRuntime } from './runtime.js';
import { search } from '../../search.js';
import { extract } from '../../extract.js';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { generateText } from 'ai';
import { config } from 'dotenv';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '../../../..');

// Load .env from project root
config({ path: resolve(projectRoot, '.env') });

const apiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
if (!apiKey) {
  console.error('ERROR: No Google API key found. Set GOOGLE_API_KEY or GOOGLE_GENERATIVE_AI_API_KEY');
  process.exit(1);
}

console.log('API key found, initializing...\n');

// Create Google provider
const google = createGoogleGenerativeAI({ apiKey });

// Create real LLM call function
async function llmCall(instruction, data, options = {}) {
  const prompt = typeof data === 'string' ? data : JSON.stringify(data, null, 2);
  const result = await generateText({
    model: google('gemini-2.5-flash'),
    system: instruction,
    prompt: prompt.substring(0, 100000),
    temperature: options.temperature || 0.3,
    maxTokens: options.maxTokens || 4000,
  });
  return result.text;
}

// The cwd for search operations
const cwd = projectRoot;

// Create real tool implementations
const toolImplementations = {
  search: {
    execute: async (params) => {
      try {
        return await search({
          query: params.query,
          path: params.path || cwd,
          cwd,
          maxTokens: 20000,
          timeout: 30,
          exact: params.exact || false,
        });
      } catch (e) {
        return `Search error: ${e.message}`;
      }
    },
  },
  extract: {
    execute: async (params) => {
      try {
        return await extract({
          targets: params.targets,
          input_content: params.input_content,
          cwd,
        });
      } catch (e) {
        return `Extract error: ${e.message}`;
      }
    },
  },
  listFiles: {
    execute: async (params) => {
      try {
        return await search({
          query: params.pattern || '*',
          path: cwd,
          cwd,
          filesOnly: true,
          maxTokens: 10000,
        });
      } catch (e) {
        return `listFiles error: ${e.message}`;
      }
    },
  },
};

// Create the DSL runtime
const runtime = createDSLRuntime({
  toolImplementations,
  llmCall,
  mapConcurrency: 3,
});

// ── Test helpers ──
let testNum = 0;
let passed = 0;
let failed = 0;

async function runTest(name, code, check) {
  testNum++;
  const label = `Test ${testNum}: ${name}`;
  console.log(`\n${'─'.repeat(70)}`);
  console.log(`▶ ${label}`);
  const codePreview = code.trim().split('\n').map(l => l.trim()).filter(Boolean).join(' ').substring(0, 140);
  console.log(`  Code: ${codePreview}...`);

  const start = Date.now();
  try {
    const result = await runtime.execute(code, name);
    const elapsed = Date.now() - start;

    const checkResult = check(result);
    if (checkResult === true || checkResult === undefined) {
      console.log(`  ✓ PASSED (${elapsed}ms)`);
      if (result.status === 'error') {
        console.log(`  (Expected error: ${result.error.substring(0, 120)})`);
      } else {
        const preview = typeof result.result === 'string'
          ? result.result.substring(0, 300)
          : JSON.stringify(result.result, null, 2).substring(0, 300);
        console.log(`  Result preview: ${preview}${preview.length >= 300 ? '...' : ''}`);
      }
      if (result.logs && result.logs.filter(l => !l.startsWith('[runtime]')).length) {
        console.log(`  Logs: ${result.logs.filter(l => !l.startsWith('[runtime]')).join(' | ')}`);
      }
      passed++;
    } else {
      console.log(`  ✗ FAILED (${elapsed}ms) — ${checkResult}`);
      if (result.logs && result.logs.length) {
        console.log(`  Logs: ${result.logs.join(' | ')}`);
      }
      failed++;
    }
  } catch (e) {
    console.log(`  ✗ CRASHED — ${e.message}`);
    console.log(`  Stack: ${e.stack?.split('\n').slice(0, 3).join(' ')}`);
    failed++;
  }
}

// ── Tests ──
async function main() {
  console.log('═'.repeat(70));
  console.log('  DSL Runtime — Complex Manual Tests');
  console.log('═'.repeat(70));

  // ────────────────────────────────────────────────
  // SECTION 1: Basic sanity
  // ────────────────────────────────────────────────

  await runTest(
    'Pure computation',
    'const x = [1,2,3,4,5]; return x.filter(n => n > 2).length;',
    (r) => r.result === 3 || `Expected 3, got ${r.result}`
  );

  await runTest(
    'Validation: rejects eval()',
    'eval("console.log(1)");',
    (r) => r.status === 'error' ? true : `Expected error, got success`
  );

  // ────────────────────────────────────────────────
  // SECTION 2: While loops & pagination simulation
  // ────────────────────────────────────────────────

  await runTest(
    'While loop: accumulate until condition',
    `
      const pages = [];
      let page = 0;
      while (page < 5) {
        pages.push({ page: page, items: range(page * 10, page * 10 + 10) });
        page = page + 1;
      }
      log("Collected " + pages.length + " pages");
      return pages.length;
    `,
    (r) => r.result === 5 || `Expected 5, got ${r.result}`
  );

  await runTest(
    'While loop with break: simulated pagination',
    `
      const allItems = [];
      let page = 1;
      while (true) {
        // Simulate a paginated API that returns 3 pages of data
        const pageData = range((page - 1) * 5, page * 5);
        const hasMore = page < 3;
        for (const item of pageData) {
          allItems.push(item);
        }
        log("Page " + page + ": " + pageData.length + " items, hasMore=" + hasMore);
        if (!hasMore) break;
        page = page + 1;
      }
      return allItems;
    `,
    (r) => {
      if (!Array.isArray(r.result)) return `Expected array, got ${typeof r.result}`;
      if (r.result.length !== 15) return `Expected 15 items, got ${r.result.length}`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 3: Try/catch error handling
  // ────────────────────────────────────────────────

  await runTest(
    'Try/catch: graceful error recovery',
    `
      const results = [];
      const queries = ["validateDSL", "thisQueryWillProbablyReturnNothing12345xyz"];
      for (const q of queries) {
        try {
          const r = search(q);
          results.push({ query: q, found: true, length: r.length });
        } catch (e) {
          results.push({ query: q, found: false, error: "failed" });
        }
      }
      return results;
    `,
    (r) => {
      if (!Array.isArray(r.result)) return `Expected array, got ${typeof r.result}`;
      if (r.result.length !== 2) return `Expected 2 results, got ${r.result.length}`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 4: Multi-search & data aggregation
  // ────────────────────────────────────────────────

  await runTest(
    'Multi-search: combine results from multiple queries',
    `
      const queries = ["error handling", "validation", "timeout"];
      const searchResults = map(queries, (q) => {
        const r = search(q);
        return { query: q, resultLength: r.length };
      });
      log("Searched " + searchResults.length + " queries");
      const totalChars = searchResults.reduce((sum, r) => sum + r.resultLength, 0);
      log("Total result chars: " + totalChars);
      return { queries: searchResults, totalChars: totalChars };
    `,
    (r) => {
      if (!r.result.queries) return `Expected queries array`;
      if (r.result.queries.length !== 3) return `Expected 3 query results`;
      if (r.result.totalChars < 100) return `Expected substantial results`;
      return true;
    }
  );

  await runTest(
    'Search + extract: find code then extract specific files',
    `
      const searchResult = search("transformDSL");
      // Extract the transformer file specifically
      const code = extract({ targets: "npm/src/agent/dsl/transformer.js" });
      const summary = LLM(
        "How many functions are exported from this file? List their names. Be very concise.",
        code
      );
      return summary;
    `,
    (r) => {
      if (typeof r.result !== 'string') return `Expected string, got ${typeof r.result}`;
      if (r.result.length < 10) return `Summary too short: ${r.result}`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 5: Complex data transformation
  // ────────────────────────────────────────────────

  await runTest(
    'Complex data pipeline: group, transform, aggregate',
    `
      // Simulate analyzing a batch of items with different categories
      const items = [];
      for (let i = 0; i < 20; i = i + 1) {
        const categories = ["bug", "feature", "docs", "refactor"];
        const priorities = ["high", "medium", "low"];
        items.push({
          id: i,
          category: categories[i % 4],
          priority: priorities[i % 3],
          title: "Item " + i
        });
      }

      // Group by category
      const byCategory = groupBy(items, "category");

      // Count per category
      const categoryNames = ["bug", "feature", "docs", "refactor"];
      const counts = [];
      for (const cat of categoryNames) {
        const count = byCategory[cat] ? byCategory[cat].length : 0;
        const highCount = byCategory[cat]
          ? byCategory[cat].filter((item) => item.priority === "high").length
          : 0;
        counts.push({ category: cat, total: count, high: highCount });
        log(cat + ": " + count + " total, " + highCount + " high priority");
      }

      return { counts: counts, totalItems: items.length };
    `,
    (r) => {
      if (r.status === 'error') return `Execution error: ${r.error}`;
      if (!r.result) return `Result is falsy: ${JSON.stringify(r)}`;
      // Debug: show what we got
      if (r.result.totalItems !== 20) return `Expected 20 total items, got type=${typeof r.result} value=${JSON.stringify(r.result).substring(0, 300)}`;
      if (!Array.isArray(r.result.counts)) return `Expected counts array, got ${JSON.stringify(r.result).substring(0, 300)}`;
      const bugs = r.result.counts.find((c) => c.category === 'bug');
      if (!bugs || bugs.total !== 5) return `Expected 5 bugs`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 6: Nested map() and LLM chaining
  // ────────────────────────────────────────────────

  await runTest(
    'Nested processing: search multiple topics, classify each result',
    `
      const topics = ["error handling", "caching"];

      // For each topic: search, then have LLM extract key patterns
      const analysis = map(topics, (topic) => {
        const results = search(topic);
        const patterns = LLM(
          "From this code, extract exactly 3 key patterns related to '" + topic + "'. " +
          "Return a brief bullet list, one pattern per line.",
          results
        );
        return { topic: topic, patterns: patterns };
      });

      log("Analyzed " + analysis.length + " topics");
      return analysis;
    `,
    (r) => {
      if (r.status === 'error') return `Execution error: ${r.error}`;
      if (!Array.isArray(r.result)) return `Expected array, got ${typeof r.result}`;
      if (r.result.length !== 2) return `Expected 2 topics analyzed`;
      // patterns is a string from LLM, not parsed
      if (typeof r.result[0].topic !== 'string') return `Missing topic`;
      if (typeof r.result[0].patterns !== 'string') return `Expected patterns to be string, got ${typeof r.result[0].patterns}`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 7: Real-world scenario — code review pipeline
  // ────────────────────────────────────────────────

  await runTest(
    'Code review pipeline: find, chunk, analyze, synthesize',
    `
      // Step 1: Search for the validator module
      const code = search("validateDSL ALLOWED_NODE_TYPES BLOCKED_IDENTIFIERS");

      // Step 2: Chunk if needed
      const codeChunks = chunk(code, 8000);
      log("Code split into " + codeChunks.length + " chunks");

      // Step 3: Analyze each chunk for issues
      const reviews = map(codeChunks, (c) => LLM(
        "You are a senior code reviewer. Analyze this code for potential issues: " +
        "security concerns, edge cases, performance problems. " +
        "Return a JSON object with: { issues: [{ severity: 'high'|'medium'|'low', description: string }] }. " +
        "Return ONLY JSON.",
        c
      ));

      // Step 4: Synthesize
      const synthesis = LLM(
        "Combine these code review findings into a prioritized summary. " +
        "Group by severity (high, medium, low). Be concise — max 5 bullet points total.",
        reviews.join("\\n---\\n")
      );

      return synthesis;
    `,
    (r) => {
      if (typeof r.result !== 'string') return `Expected string`;
      if (r.result.length < 50) return `Review too short`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 8: Real-world — dependency analysis
  // ────────────────────────────────────────────────

  await runTest(
    'Dependency analysis: find imports across multiple files',
    `
      // Search for all imports in the DSL module files
      const files = ["validator.js", "transformer.js", "environment.js", "runtime.js"];
      const imports = map(files, (file) => {
        const code = extract({ targets: "npm/src/agent/dsl/" + file });
        const analysis = LLM(
          "List all import statements from this file. Return a JSON object: " +
          "{ file: string, imports: [{ from: string, names: string[] }] }. Return ONLY JSON.",
          code
        );
        return analysis;
      });

      log("Analyzed " + imports.length + " files");

      // Have LLM create a dependency graph summary
      const summary = LLM(
        "Given these import analyses for DSL module files, create a brief dependency summary: " +
        "which files depend on what external packages and internal modules. " +
        "Format as a simple list. Be concise.",
        imports.join("\\n")
      );

      return summary;
    `,
    (r) => {
      if (typeof r.result !== 'string') return `Expected string`;
      if (r.result.length < 30) return `Summary too short`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 9: Stress test — many parallel LLM calls
  // ────────────────────────────────────────────────

  await runTest(
    'Stress: 10 parallel LLM calls via map()',
    `
      const items = range(1, 11);
      const results = map(items, (n) => {
        const answer = LLM(
          "Return ONLY a single number: the square of " + n + ". Nothing else, just the number.",
          "Calculate " + n + " * " + n
        );
        return { n: n, squared: String(answer).trim() };
      });
      log("Completed " + results.length + " parallel LLM calls");
      return results;
    `,
    (r) => {
      if (r.status === 'error') return `Execution error: ${r.error}`;
      if (!Array.isArray(r.result)) return `Expected array, got ${typeof r.result}`;
      if (r.result.length !== 10) return `Expected 10 results, got ${r.result.length}`;
      const first = r.result[0];
      if (first.n === undefined || first.squared === undefined) return `Missing fields: ${JSON.stringify(first)}`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 10: Complex conditional logic
  // ────────────────────────────────────────────────

  await runTest(
    'Conditional routing: different processing based on search results',
    `
      const queries = ["BLOCKED_IDENTIFIERS", "nonexistent_symbol_xyz_12345"];
      const results = [];

      for (const q of queries) {
        const searchResult = search(q);

        if (searchResult.length > 500) {
          // Rich results — summarize
          const summary = LLM("Summarize this code in one sentence.", searchResult);
          results.push({ query: q, status: "found", summary });
        } else if (searchResult.length > 100) {
          // Some results — note them
          results.push({ query: q, status: "partial", chars: searchResult.length });
        } else {
          // No meaningful results
          results.push({ query: q, status: "not_found" });
        }
        log(q + " -> " + results[results.length - 1].status);
      }

      return results;
    `,
    (r) => {
      if (!Array.isArray(r.result)) return `Expected array`;
      if (r.result.length !== 2) return `Expected 2 results`;
      if (r.result[0].status !== 'found') return `First query should be 'found'`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 11: While + search iteration (paginated search simulation)
  // ────────────────────────────────────────────────

  await runTest(
    'Iterative deepening: search, then search within results',
    `
      // First broad search
      const broad = search("sandbox");
      const broadSummary = LLM(
        "From these search results, identify the 2 most important function names " +
        "related to sandboxing. Return ONLY the function names separated by comma.",
        broad
      );
      log("Broad search found key functions: " + broadSummary);

      // Now search specifically for each function
      const parts = broadSummary.split(",");
      const functions = [];
      for (const p of parts) {
        const trimmed = p.trim();
        if (trimmed.length > 0) functions.push(trimmed);
      }
      log("Will search for " + functions.length + " functions");

      const details = map(functions.slice(0, 2), (fn) => {
        const detail = search(fn);
        const analysis = LLM(
          "Explain what the function '" + fn + "' does in 1-2 sentences based on this code.",
          detail
        );
        return { name: fn, description: analysis };
      });

      return details;
    `,
    (r) => {
      if (r.status === 'error') return `Execution error: ${r.error}`;
      if (!Array.isArray(r.result)) return `Expected array, got ${typeof r.result}: ${JSON.stringify(r.result).substring(0, 200)}`;
      if (r.result.length < 1) return `Expected at least 1 function analyzed`;
      if (!r.result[0].description) return `Missing description`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 12: Full analyze_all replacement pattern
  // ────────────────────────────────────────────────

  await runTest(
    'analyze_all replacement: comprehensive codebase question',
    `
      // Question: "What testing patterns are used in the DSL module?"

      // Phase 1: Search for test-related code
      const testResults = search("test DSL validator transformer runtime");

      // Phase 2: Chunk and extract patterns
      const chunks = chunk(testResults, 6000);
      log("Processing " + chunks.length + " test chunks");

      const patterns = map(chunks, (c) => LLM(
        "Extract testing patterns from this code. For each pattern found, note: " +
        "1) Pattern name (e.g., 'mock functions', 'assertion style', 'test structure') " +
        "2) Brief description " +
        "Return as a bullet list. Be concise.",
        c
      ));

      // Phase 3: Synthesize
      const answer = LLM(
        "You are answering the question: 'What testing patterns are used in the DSL module?' " +
        "Based on the analysis below, provide a comprehensive but concise answer. " +
        "Organize by pattern type. Use bullet points. Max 10 bullet points.",
        patterns.join("\\n---\\n")
      );

      return answer;
    `,
    (r) => {
      if (typeof r.result !== 'string') return `Expected string`;
      if (r.result.length < 100) return `Answer too short`;
      return true;
    }
  );

  // ────────────────────────────────────────────────
  // SECTION 13: Discovery-first pattern
  // ────────────────────────────────────────────────

  await runTest(
    'Discovery-first: explore repo then plan search strategy',
    `
      // Phase 1: Discover repo structure
      const files = listFiles("**/*");
      const sample = search("error handling");
      log("Files length: " + String(files).length + ", sample length: " + String(sample).length);

      // Phase 2: Ask LLM to determine optimal search strategy
      const plan = LLM(
        "Based on this repository structure and sample search results, determine the best search strategy " +
        "to answer: 'What are all the validation approaches in this codebase?' " +
        "Return a JSON object with: keywords (array of 2-3 search queries that will find relevant data), " +
        "extractionFocus (what to extract from each result), " +
        "and aggregation (summarize or list_unique). " +
        "IMPORTANT: Only suggest keywords likely to match actual content you see. Return ONLY valid JSON.",
        "Repository files:\\n" + String(files).substring(0, 3000) + "\\nSample results:\\n" + String(sample).substring(0, 3000)
      );
      const strategy = JSON.parse(String(plan));
      log("Strategy keywords: " + strategy.keywords.length + ", focus: " + strategy.extractionFocus);

      // Phase 3: Execute with discovered strategy
      const allFindings = [];
      for (const kw of strategy.keywords) {
        const results = search(kw);
        if (String(results).length > 500) {
          const chunks = chunk(results);
          const findings = map(chunks, (c) => LLM(strategy.extractionFocus, c));
          for (const f of findings) { allFindings.push(String(f)); }
          log("Keyword '" + kw + "': " + chunks.length + " chunks processed");
        } else {
          log("Keyword '" + kw + "': skipped (too few results)");
        }
      }
      var combined = "";
      for (const f of allFindings) { combined = combined + f + "\\n---\\n"; }
      return LLM("Synthesize all findings about validation approaches into a comprehensive answer.", combined);
    `,
    (r) => {
      if (typeof r.result !== 'string') return `Expected string`;
      if (r.result.length < 100) return `Answer too short: ${r.result.length} chars`;
      return true;
    }
  );

  // ── Summary ──
  console.log(`\n${'═'.repeat(70)}`);
  console.log(`  Results: ${passed} passed, ${failed} failed, ${testNum} total`);
  console.log('═'.repeat(70));

  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => {
  console.error('Fatal error:', e);
  process.exit(1);
});
