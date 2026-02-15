#!/usr/bin/env node
/**
 * Real-world test of the analyze_all replacement pattern.
 *
 * Tests against the TykTechnologies/customer-insights repo (582 markdown files, 16MB)
 * to verify the search → chunk → map(LLM) → synthesize pipeline works at scale.
 *
 * Usage:
 *   node npm/src/agent/dsl/analyze-test.mjs
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

config({ path: resolve(projectRoot, '.env') });

const apiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
if (!apiKey) {
  console.error('ERROR: No Google API key found.');
  process.exit(1);
}

const google = createGoogleGenerativeAI({ apiKey });

async function llmCall(instruction, data, options = {}) {
  const dataStr = data == null ? '' : (typeof data === 'string' ? data : JSON.stringify(data, null, 2));
  const prompt = (dataStr || '(empty)').substring(0, 100000);
  const result = await generateText({
    model: google('gemini-2.5-flash'),
    system: instruction,
    prompt,
    temperature: options.temperature || 0.3,
    maxTokens: options.maxTokens || 4000,
  });
  return result.text;
}

const TARGET_REPO = '/tmp/customer-insights';

const toolImplementations = {
  search: {
    execute: async (params) => {
      try {
        return await search({
          query: params.query,
          path: params.path || TARGET_REPO,
          cwd: TARGET_REPO,
          maxTokens: 20000,
          timeout: 30,
          exact: params.exact || false,
        });
      } catch (e) {
        return "Search error: " + e.message;
      }
    },
  },
  extract: {
    execute: async (params) => {
      try {
        return await extract({
          targets: params.targets,
          input_content: params.input_content,
          cwd: TARGET_REPO,
        });
      } catch (e) {
        return "Extract error: " + e.message;
      }
    },
  },
};

const runtime = createDSLRuntime({
  toolImplementations,
  llmCall,
  mapConcurrency: 3,
  timeoutMs: 120000,
  maxLoopIterations: 5000,
});

// ── Tests ──
let testNum = 0;
let passed = 0;
let failed = 0;

async function runTest(name, code, check) {
  testNum++;
  console.log(`\n${'─'.repeat(70)}`);
  console.log(`▶ Test ${testNum}: ${name}`);
  console.log(`  Code (${code.trim().split('\n').length} lines):`);
  const preview = code.trim().split('\n').slice(0, 8).map(l => '    ' + l.trim()).join('\n');
  console.log(preview);
  if (code.trim().split('\n').length > 8) console.log('    ...');

  const start = Date.now();
  try {
    const result = await runtime.execute(code, name);
    const elapsed = Date.now() - start;

    if (result.status === 'error') {
      console.log(`  ✗ EXECUTION ERROR (${elapsed}ms)`);
      console.log(`  Error: ${result.error.substring(0, 300)}`);
      if (result.logs.length) console.log(`  Logs: ${result.logs.join(' | ')}`);
      failed++;
      return;
    }

    const userLogs = result.logs.filter(l => !l.startsWith('[runtime]'));
    if (userLogs.length) {
      console.log(`  Logs: ${userLogs.join(' | ')}`);
    }

    const checkResult = check(result);
    if (checkResult === true) {
      console.log(`  ✓ PASSED (${elapsed}ms)`);
      const resultStr = typeof result.result === 'string'
        ? result.result.substring(0, 500)
        : JSON.stringify(result.result, null, 2).substring(0, 500);
      console.log(`  Result: ${resultStr}${resultStr.length >= 500 ? '...' : ''}`);
      passed++;
    } else {
      console.log(`  ✗ CHECK FAILED (${elapsed}ms) — ${checkResult}`);
      failed++;
    }
  } catch (e) {
    const elapsed = Date.now() - start;
    console.log(`  ✗ CRASHED (${elapsed}ms) — ${e.message}`);
    failed++;
  }
}

async function main() {
  console.log('═'.repeat(70));
  console.log('  analyze_all Replacement — Real-World Tests');
  console.log('  Target: TykTechnologies/customer-insights (582 .md files, 16MB)');
  console.log('═'.repeat(70));

  // Test 1: Core analyze_all pattern — search → chunk → map(LLM) → synthesize
  await runTest(
    'analyze_all pattern: "api governance"',
    `
      const results = search("api governance");
      log("Search returned " + String(results).length + " chars");
      const chunks = chunk(results);
      log("Split into " + chunks.length + " chunks");
      const extracted = map(chunks, (c) => LLM("List every mention of API governance — who uses it, what for, any specific policies or tools mentioned. Be brief and factual.", c));
      var combined = "";
      for (const e of extracted) { combined = combined + String(e) + "\\n---\\n"; }
      return LLM("Synthesize into a comprehensive report about API governance across all customers. Group by: 1) Customers using API governance, 2) Governance tools/approaches, 3) Common patterns. Be thorough.", combined);
    `,
    (r) => {
      if (typeof r.result !== 'string') return 'Expected string result';
      if (r.result.length < 100) return 'Result too short: ' + r.result.length;
      return true;
    }
  );

  // Test 2: Multi-topic search — governance + rate limiting + security
  await runTest(
    'Multi-topic: governance, rate limiting, security policies',
    `
      const topics = ["api governance", "rate limiting", "security policy"];
      const allFindings = [];
      for (const topic of topics) {
        const results = search(topic);
        log(topic + ": " + String(results).length + " chars");
        const chunks = chunk(results);
        const findings = map(chunks, (c) => LLM("Extract key findings about " + topic + ". Include customer names and specifics. Be brief.", c));
        for (const f of findings) { allFindings.push(topic + ": " + String(f)); }
      }
      var combined = "";
      for (const f of allFindings) { combined = combined + f + "\\n---\\n"; }
      return LLM("Create a cross-topic analysis: How do customers approach API governance, rate limiting, and security together? What patterns emerge?", combined);
    `,
    (r) => {
      if (typeof r.result !== 'string') return 'Expected string result';
      if (r.result.length < 100) return 'Result too short';
      return true;
    }
  );

  // Test 3: Extract specific data points
  await runTest(
    'Extract customer use cases for API management',
    `
      const results = search("use case API management");
      log("Search: " + String(results).length + " chars");
      const chunks = chunk(results);
      log("Chunks: " + chunks.length);
      const extracted = map(chunks, (c) => LLM("Extract a JSON array of objects with fields: customer (string), use_case (string), outcome (string or null). Only include clearly stated use cases. Return valid JSON array only.", c));
      var allUseCases = [];
      for (const e of extracted) {
        try {
          var text = String(e).trim();
          var jsonStart = text.indexOf("[");
          var jsonEnd = text.lastIndexOf("]");
          if (jsonStart >= 0 && jsonEnd > jsonStart) {
            text = text.substring(jsonStart, jsonEnd + 1);
          }
          var parsed = JSON.parse(text);
          if (Array.isArray(parsed)) {
            for (const item of parsed) { allUseCases.push(item); }
          }
        } catch (err) {
          log("Parse failed for chunk, skipping");
        }
      }
      log("Total use cases found: " + allUseCases.length);
      return allUseCases;
    `,
    (r) => {
      if (!Array.isArray(r.result)) return 'Expected array result';
      if (r.result.length === 0) return 'No use cases extracted';
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
