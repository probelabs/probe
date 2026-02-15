#!/usr/bin/env node
/**
 * Trigger test: verifies that the agent picks execute_plan for the right queries.
 *
 * Runs the real ProbeAgent with enableExecutePlan=true and observes which tools
 * get called for different types of questions. This tests the tool-selection
 * logic end-to-end — the system prompt, tool descriptions, and LLM decision-making.
 *
 * Usage:
 *   node npm/src/agent/dsl/trigger-test.mjs
 *
 * Requires: GOOGLE_API_KEY or GOOGLE_GENERATIVE_AI_API_KEY in .env
 */

import { ProbeAgent } from '../ProbeAgent.js';
import { config } from 'dotenv';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '../../../..');

config({ path: resolve(projectRoot, '.env') });

// Check for API key
const apiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
if (!apiKey) {
  console.error('ERROR: No Google API key found. Set GOOGLE_API_KEY or GOOGLE_GENERATIVE_AI_API_KEY');
  process.exit(1);
}

// ── Test definitions ──
// Each test has a query and an expected tool choice
const tests = [
  // ── Should trigger execute_plan ──
  {
    name: 'Aggregate question (all patterns)',
    query: 'Find ALL error handling patterns across the entire codebase and give me a comprehensive summary covering every module.',
    expectTool: 'execute_plan',
    reason: 'Aggregate question needing full data coverage + "ALL" + "comprehensive" + "every module"',
  },
  {
    name: 'Multi-topic bulk scan',
    query: 'Search for authentication, authorization, and session management patterns. Analyze each topic across the full codebase and produce a security report.',
    expectTool: 'execute_plan',
    reason: 'Multiple topics + full codebase scan + synthesis',
  },
  {
    name: 'Open-ended discovery',
    query: 'What are all the different testing approaches used in this codebase? Give me a complete inventory.',
    expectTool: 'execute_plan',
    reason: 'Open-ended, needs discovery + comprehensive scan',
  },

  // ── Should NOT trigger execute_plan ──
  {
    name: 'Simple search (specific function)',
    query: 'How does the validateDSL function work?',
    expectTool: 'search',
    reason: 'Specific function lookup, 1-2 tool calls',
  },
  {
    name: 'Simple search (single concept)',
    query: 'What is the timeout configuration for the DSL runtime?',
    expectTool: 'search',
    reason: 'Narrow question, single concept',
  },
];

// ── Test runner ──
let testNum = 0;
let passed = 0;
let failed = 0;

async function runTriggerTest(test) {
  testNum++;
  console.log(`\n${'─'.repeat(70)}`);
  console.log(`▶ Test ${testNum}: ${test.name}`);
  console.log(`  Query: "${test.query.substring(0, 100)}${test.query.length > 100 ? '...' : ''}"`);
  console.log(`  Expected tool: ${test.expectTool}`);

  const toolCalls = [];

  const agent = new ProbeAgent({
    path: projectRoot,
    provider: 'google',
    model: 'gemini-2.5-flash',
    enableExecutePlan: true,
    maxIterations: 3, // Only need first few iterations to see what tool gets picked
  });

  // Listen for tool call events
  agent.events.on('toolCall', (event) => {
    if (event.status === 'started') {
      toolCalls.push(event.name);
      console.log(`  [tool] ${event.name}`);
    }
  });

  await agent.initialize();

  const start = Date.now();
  try {
    await agent.answer(test.query);
  } catch (e) {
    // May hit maxIterations limit — that's fine, we just want tool selection
    if (!e.message?.includes('iteration') && !e.message?.includes('cancelled')) {
      console.log(`  [warn] Agent error: ${e.message?.substring(0, 150)}`);
    }
  }

  const elapsed = Date.now() - start;
  const firstMeaningfulTool = toolCalls.find(t =>
    t === 'execute_plan' || t === 'analyze_all' || t === 'search' || t === 'query'
  );

  console.log(`  All tool calls: [${toolCalls.join(', ')}]`);
  console.log(`  First meaningful tool: ${firstMeaningfulTool || '(none)'}`);

  const toolMatch = firstMeaningfulTool === test.expectTool;

  if (toolMatch) {
    console.log(`  ✓ PASSED — picked ${firstMeaningfulTool} as expected (${elapsed}ms)`);
    passed++;
  } else {
    console.log(`  ✗ FAILED — expected ${test.expectTool}, got ${firstMeaningfulTool || '(none)'} (${elapsed}ms)`);
    console.log(`  Reason it should use ${test.expectTool}: ${test.reason}`);
    failed++;
  }

  try {
    await agent.close();
  } catch (e) {
    // ignore cleanup errors
  }
}

async function main() {
  console.log('═'.repeat(70));
  console.log('  Execute Plan Trigger Tests — Tool Selection Verification');
  console.log('═'.repeat(70));
  console.log(`\nRunning with: enableExecutePlan=true, provider=google, model=gemini-2.5-flash`);
  console.log(`Project root: ${projectRoot}`);

  for (const test of tests) {
    await runTriggerTest(test);
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
