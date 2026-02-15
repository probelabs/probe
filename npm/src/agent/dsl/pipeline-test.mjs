#!/usr/bin/env node
/**
 * Data pipeline end-to-end test using ProbeAgent with enableExecutePlan.
 *
 * Tests against the TykTechnologies/customer-insights repo (/tmp/customer-insights)
 * to verify the full data pipeline flow:
 *   1. Agent picks execute_plan for comprehensive/inventory questions
 *   2. LLM generates DSL scripts with search → chunk → LLM classify → accumulate
 *   3. Session store persists data across multi-step execution
 *   4. Returns structured results (tables, JSON, reports)
 *
 * Usage:
 *   node npm/src/agent/dsl/pipeline-test.mjs
 *
 * Requires:
 *   - GOOGLE_API_KEY or GOOGLE_GENERATIVE_AI_API_KEY in .env
 *   - /tmp/customer-insights repo cloned
 */

import { ProbeAgent } from '../ProbeAgent.js';
import { config } from 'dotenv';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { existsSync } from 'fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '../../../..');

config({ path: resolve(projectRoot, '.env') });

const apiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
if (!apiKey) {
  console.error('ERROR: No Google API key found. Set GOOGLE_API_KEY or GOOGLE_GENERATIVE_AI_API_KEY');
  process.exit(1);
}

const TARGET_REPO = '/tmp/customer-insights';
if (!existsSync(TARGET_REPO)) {
  console.error('ERROR: customer-insights repo not found at ' + TARGET_REPO);
  console.error('Clone it: git clone <repo-url> /tmp/customer-insights');
  process.exit(1);
}

// ── Test definitions ──
const tests = [
  {
    name: 'Customer classification — categorize all customers by industry/type',
    query: 'Analyze ALL customer files in this repository. For every customer, classify them by industry (finance, tech, healthcare, government, etc.) and determine their use case type (API management, security, integration, etc.). Produce a comprehensive markdown table with columns: Customer, Industry, Use Case Type, and a brief note. Give me complete inventory.',
    maxIterations: 50,
    timeoutMs: 300000,
    check: (result, toolCalls) => {
      // Should have triggered execute_plan
      const usedExecutePlan = toolCalls.some(t => t === 'execute_plan');
      if (!usedExecutePlan) return 'Did not trigger execute_plan — used: ' + toolCalls.join(', ');
      // Result should be substantial
      if (!result || result.length < 200) return 'Result too short: ' + (result?.length || 0);
      return true;
    },
  },
  {
    name: 'Sentiment & pain points extraction — data pipeline pattern',
    query: 'Go through every customer document in this repo. For each customer, extract their main pain points and sentiment (positive, neutral, negative) about Tyk. Produce a structured report with: 1) A summary table of sentiment distribution, 2) Top 5 most common pain points with customer counts, 3) Customers with negative sentiment and why. Be comprehensive — cover ALL customers.',
    maxIterations: 50,
    timeoutMs: 300000,
    check: (result, toolCalls) => {
      const usedExecutePlan = toolCalls.some(t => t === 'execute_plan');
      if (!usedExecutePlan) return 'Did not trigger execute_plan';
      if (!result || result.length < 200) return 'Result too short: ' + (result?.length || 0);
      return true;
    },
  },
  {
    name: 'Feature adoption matrix — multi-search data pipeline',
    query: 'Create a complete feature adoption matrix for this customer base. Search for mentions of: API gateway, dashboard, developer portal, analytics, rate limiting, authentication, policies, and GraphQL. For each feature, list which customers use it. Return a markdown table where rows are features and columns show customer count + list of customer names.',
    maxIterations: 50,
    timeoutMs: 300000,
    check: (result, toolCalls) => {
      const usedExecutePlan = toolCalls.some(t => t === 'execute_plan');
      if (!usedExecutePlan) return 'Did not trigger execute_plan';
      if (!result || result.length < 100) return 'Result too short: ' + (result?.length || 0);
      return true;
    },
  },
];

// ── Test runner ──
let testNum = 0;
let passed = 0;
let failed = 0;

async function runPipelineTest(test) {
  testNum++;
  console.log(`\n${'═'.repeat(70)}`);
  console.log(`▶ Test ${testNum}/${tests.length}: ${test.name}`);
  console.log(`  Query: "${test.query.substring(0, 120)}..."`);
  console.log('─'.repeat(70));

  const toolCalls = [];
  const toolDetails = [];

  const agent = new ProbeAgent({
    path: TARGET_REPO,
    provider: 'google',
    model: 'gemini-2.5-flash',
    enableExecutePlan: true,
    maxIterations: test.maxIterations || 50,
  });

  // Listen for tool call events
  agent.events.on('toolCall', (event) => {
    if (event.status === 'started') {
      toolCalls.push(event.name);
      const desc = event.description ? ` — ${event.description.substring(0, 80)}` : '';
      console.log(`  [tool:start] ${event.name}${desc}`);
    }
    if (event.status === 'completed') {
      const preview = event.resultPreview || '';
      console.log(`  [tool:done]  ${event.name} (${String(preview).length} chars preview)`);
    }
    if (event.status === 'error') {
      console.log(`  [tool:error] ${event.name}: ${event.error?.substring(0, 100)}`);
    }
  });

  await agent.initialize();

  const start = Date.now();
  let result;
  try {
    result = await Promise.race([
      agent.answer(test.query),
      new Promise((_, reject) =>
        setTimeout(() => reject(new Error('Test timeout')), test.timeoutMs || 180000)
      ),
    ]);
  } catch (e) {
    const elapsed = Math.round((Date.now() - start) / 1000);
    console.log(`\n  [warn] Agent finished with: ${e.message?.substring(0, 150)} (${elapsed}s)`);
    // Still check what we got — agent may have partial result
    result = e.message;
  }

  const elapsed = Math.round((Date.now() - start) / 1000);

  console.log('─'.repeat(70));
  console.log(`  Duration: ${elapsed}s`);
  console.log(`  Tool calls: [${toolCalls.join(', ')}]`);
  console.log(`  execute_plan used: ${toolCalls.includes('execute_plan') ? 'YES' : 'NO'}`);

  const resultStr = typeof result === 'string' ? result : JSON.stringify(result);
  console.log(`  Result length: ${resultStr?.length || 0} chars`);

  // Show result preview
  if (resultStr) {
    console.log('─'.repeat(70));
    console.log('  Result preview:');
    const lines = resultStr.split('\n').slice(0, 25);
    for (const line of lines) {
      console.log('  │ ' + line.substring(0, 100));
    }
    if (resultStr.split('\n').length > 25) {
      console.log('  │ ... (' + (resultStr.split('\n').length - 25) + ' more lines)');
    }
  }

  // Run check
  const checkResult = test.check(resultStr, toolCalls);
  if (checkResult === true) {
    console.log(`\n  ✓ PASSED (${elapsed}s)`);
    passed++;
  } else {
    console.log(`\n  ✗ FAILED — ${checkResult} (${elapsed}s)`);
    failed++;
  }

  // Token usage
  try {
    const usage = agent.getTokenUsage();
    if (usage) {
      console.log(`  Tokens: input=${usage.inputTokens || 0} output=${usage.outputTokens || 0} total=${usage.totalTokens || 0}`);
    }
  } catch (e) {
    // ignore
  }

  try {
    await agent.close();
  } catch (e) {
    // ignore cleanup errors
  }
}

// ── Main ──
async function main() {
  console.log('═'.repeat(70));
  console.log('  Data Pipeline E2E Tests — ProbeAgent + execute_plan');
  console.log('  Target: TykTechnologies/customer-insights');
  console.log('  Config: enableExecutePlan=true, provider=google, model=gemini-2.5-flash');
  console.log('═'.repeat(70));

  // Allow running a specific test by number
  const testIndex = process.argv[2] ? parseInt(process.argv[2], 10) - 1 : null;

  if (testIndex !== null && testIndex >= 0 && testIndex < tests.length) {
    console.log(`\nRunning test ${testIndex + 1} only: "${tests[testIndex].name}"`);
    await runPipelineTest(tests[testIndex]);
  } else {
    for (const test of tests) {
      await runPipelineTest(test);
    }
  }

  console.log(`\n${'═'.repeat(70)}`);
  console.log(`  Results: ${passed} passed, ${failed} failed, ${testNum} total`);
  console.log('═'.repeat(70));

  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => {
  console.error('Fatal error:', e);
  process.exit(1);
});
