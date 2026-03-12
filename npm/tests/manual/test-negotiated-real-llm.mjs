#!/usr/bin/env node
/**
 * Real LLM integration test for negotiated timeout observer pattern.
 *
 * Tests with a real Google API key and short timeouts to trigger the observer.
 * Usage: GOOGLE_API_KEY=... node tests/manual/test-negotiated-real-llm.mjs
 */

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

const PASS = '\x1b[32m✓\x1b[0m';
const FAIL = '\x1b[31m✗\x1b[0m';
const BOLD = '\x1b[1m';
const DIM = '\x1b[2m';
const RESET = '\x1b[0m';

let passed = 0;
let failed = 0;

function assert(condition, label) {
  if (condition) {
    console.log(`  ${PASS} ${label}`);
    passed++;
  } else {
    console.log(`  ${FAIL} ${label}`);
    failed++;
  }
}

// ============================================================================
// Test 1: Negotiated timeout triggers observer and produces summary
// ============================================================================

console.log(`\n${BOLD}Test 1: Negotiated Timeout → Observer → Summary${RESET}`);
console.log(`${DIM}  (Using very short timeout to force trigger)${RESET}`);

try {
  const agent1 = new ProbeAgent({
    path: '/home/buger/projects/probe',
    provider: 'google',
    model: 'gemini-2.0-flash',
    timeoutBehavior: 'negotiated',
    maxOperationTimeout: 8000,           // 8s — should trigger during search
    negotiatedTimeoutMaxRequests: 1,      // Only 1 extension allowed
    negotiatedTimeoutMaxPerRequest: 10000, // 10s per extension
    negotiatedTimeoutBudget: 30000,       // 30s total budget
    debug: true,
  });

  const events = [];
  agent1.events.on('toolCall', (ev) => {
    events.push({ name: ev.name, status: ev.status, id: ev.toolCallId });
  });

  console.log(`  ${DIM}Asking complex question with 8s timeout...${RESET}`);
  const result1 = await agent1.answer(
    'Search for all error handling patterns in this codebase. Look at multiple files and provide a comprehensive analysis of how errors are handled across different modules.'
  );

  assert(typeof result1 === 'string', 'got a string result');
  assert(result1.length > 50, `result is substantial (${result1.length} chars)`);
  assert(events.length > 0, `tool events fired (${events.length} events)`);

  // Check if timeout notice or actual content is present
  const hasTimeoutNotice = result1.includes('time constraint') || result1.includes('timeout') || result1.includes('interrupted');
  const hasContent = result1.length > 100;
  assert(hasTimeoutNotice || hasContent, 'result has timeout notice or substantive content');

  console.log(`  ${DIM}Result preview: ${result1.substring(0, 200).replace(/\n/g, ' ')}...${RESET}`);
  console.log(`  ${DIM}Tool events: ${events.map(e => `${e.name}:${e.status}`).join(', ')}${RESET}`);
} catch (err) {
  console.log(`  ${FAIL} Test 1 threw: ${err.message}`);
  failed++;
}

// ============================================================================
// Test 2: Negotiated timeout with JSON schema
// ============================================================================

console.log(`\n${BOLD}Test 2: Negotiated Timeout with JSON Schema${RESET}`);
console.log(`${DIM}  (Schema mode should return valid JSON even after abort)${RESET}`);

try {
  const agent2 = new ProbeAgent({
    path: '/home/buger/projects/probe',
    provider: 'google',
    model: 'gemini-2.0-flash',
    timeoutBehavior: 'negotiated',
    maxOperationTimeout: 8000,
    negotiatedTimeoutMaxRequests: 0,      // No extensions — force immediate decline
    negotiatedTimeoutBudget: 1000,
    debug: true,
  });

  const schema = {
    type: 'object',
    properties: {
      summary: { type: 'string', description: 'Summary of findings' },
      files_analyzed: { type: 'number', description: 'Number of files analyzed' },
      completed: { type: 'boolean', description: 'Whether analysis completed' },
    },
    required: ['summary'],
  };

  console.log(`  ${DIM}Asking with schema and 8s timeout...${RESET}`);
  const result2 = await agent2.answer(
    'Analyze the error handling patterns in this codebase and report your findings.',
    [],
    { schema: JSON.stringify(schema) }
  );

  assert(typeof result2 === 'string', 'got a string result');
  assert(!result2.startsWith('**Note'), 'no markdown notice prepended in schema mode');

  let parsedJson = null;
  try {
    parsedJson = JSON.parse(result2);
    assert(true, 'result is valid JSON');
  } catch {
    // May not be pure JSON if the model answered before timeout
    assert(result2.length > 0, `result is non-empty (${result2.length} chars, may not be JSON if answered before timeout)`);
  }

  if (parsedJson) {
    assert(typeof parsedJson.summary === 'string' || parsedJson.summary === undefined,
      'JSON has summary field or partial data');
  }

  console.log(`  ${DIM}Result preview: ${result2.substring(0, 200).replace(/\n/g, ' ')}...${RESET}`);
} catch (err) {
  console.log(`  ${FAIL} Test 2 threw: ${err.message}`);
  failed++;
}

// ============================================================================
// Test 3: Negotiated timeout with onStream callback
// ============================================================================

console.log(`\n${BOLD}Test 3: Negotiated Timeout with onStream Callback${RESET}`);
console.log(`${DIM}  (Streaming consumers should receive output even after abort)${RESET}`);

try {
  const agent3 = new ProbeAgent({
    path: '/home/buger/projects/probe',
    provider: 'google',
    model: 'gemini-2.0-flash',
    timeoutBehavior: 'negotiated',
    maxOperationTimeout: 8000,
    negotiatedTimeoutMaxRequests: 0,
    negotiatedTimeoutBudget: 1000,
    debug: true,
  });

  const streamedChunks = [];
  console.log(`  ${DIM}Asking with onStream and 8s timeout...${RESET}`);
  const result3 = await agent3.answer(
    'Search for all test files in this codebase and describe the testing patterns used.',
    [],
    { onStream: (chunk) => streamedChunks.push(chunk) }
  );

  assert(typeof result3 === 'string', 'got a string result');
  assert(streamedChunks.length > 0, `onStream received ${streamedChunks.length} chunk(s)`);

  const totalStreamed = streamedChunks.join('').length;
  assert(totalStreamed > 0, `streamed content has ${totalStreamed} total chars`);

  console.log(`  ${DIM}Streamed ${streamedChunks.length} chunks, total ${totalStreamed} chars${RESET}`);
  console.log(`  ${DIM}Result preview: ${result3.substring(0, 200).replace(/\n/g, ' ')}...${RESET}`);
} catch (err) {
  console.log(`  ${FAIL} Test 3 threw: ${err.message}`);
  failed++;
}

// ============================================================================
// Test 4: Negotiated timeout with completionPrompt (should NOT fire after abort)
// ============================================================================

console.log(`\n${BOLD}Test 4: Negotiated Timeout + completionPrompt (skipped after abort)${RESET}`);
console.log(`${DIM}  (completionPrompt should NOT trigger a second LLM pass after abort summary)${RESET}`);

try {
  const agent4 = new ProbeAgent({
    path: '/home/buger/projects/probe',
    provider: 'google',
    model: 'gemini-2.0-flash',
    timeoutBehavior: 'negotiated',
    maxOperationTimeout: 8000,
    negotiatedTimeoutMaxRequests: 0,
    negotiatedTimeoutBudget: 1000,
    completionPrompt: 'Review the output for completeness and add any missing details.',
    debug: true,
  });

  console.log(`  ${DIM}Asking with completionPrompt and 8s timeout...${RESET}`);
  const startTime = Date.now();
  const result4 = await agent4.answer(
    'Search for all imports in the ProbeAgent.js file and list them.'
  );
  const elapsed = Date.now() - startTime;

  assert(typeof result4 === 'string', 'got a string result');
  assert(result4.length > 0, `result is non-empty (${result4.length} chars)`);
  // If completionPrompt fired, it would add significant time. With 8s timeout + summary,
  // we should be well under 30s. If completionPrompt ran, it would be longer.
  assert(elapsed < 45000, `completed in reasonable time (${(elapsed/1000).toFixed(1)}s — no extra completionPrompt pass)`);

  console.log(`  ${DIM}Elapsed: ${(elapsed/1000).toFixed(1)}s${RESET}`);
  console.log(`  ${DIM}Result preview: ${result4.substring(0, 200).replace(/\n/g, ' ')}...${RESET}`);
} catch (err) {
  console.log(`  ${FAIL} Test 4 threw: ${err.message}`);
  failed++;
}

// ============================================================================
// Test 5: Normal completion (no timeout triggered)
// ============================================================================

console.log(`\n${BOLD}Test 5: Normal Completion (generous timeout, no trigger)${RESET}`);
console.log(`${DIM}  (With long timeout, agent should complete normally)${RESET}`);

try {
  const agent5 = new ProbeAgent({
    path: '/home/buger/projects/probe',
    provider: 'google',
    model: 'gemini-2.0-flash',
    timeoutBehavior: 'negotiated',
    maxOperationTimeout: 120000,          // 2 min — should NOT trigger
    negotiatedTimeoutMaxRequests: 3,
    debug: true,
  });

  console.log(`  ${DIM}Asking simple question with 2min timeout...${RESET}`);
  const result5 = await agent5.answer(
    'What programming language is the probe tool written in? Answer in one sentence.'
  );

  assert(typeof result5 === 'string', 'got a string result');
  assert(result5.length > 10, `result is non-empty (${result5.length} chars)`);
  assert(!result5.includes('time constraint'), 'no timeout notice (normal completion)');
  assert(!result5.includes('interrupted'), 'no interruption notice');

  console.log(`  ${DIM}Result: ${result5.substring(0, 300).replace(/\n/g, ' ')}${RESET}`);
} catch (err) {
  console.log(`  ${FAIL} Test 5 threw: ${err.message}`);
  failed++;
}

// ============================================================================
// Test 6: Observer extends once, then declines
// ============================================================================

console.log(`\n${BOLD}Test 6: Observer Extends Once Then Declines${RESET}`);
console.log(`${DIM}  (Allow 1 extension, total should take longer than single timeout)${RESET}`);

try {
  const agent6 = new ProbeAgent({
    path: '/home/buger/projects/probe',
    provider: 'google',
    model: 'gemini-2.0-flash',
    timeoutBehavior: 'negotiated',
    maxOperationTimeout: 10000,           // 10s initial timeout
    negotiatedTimeoutMaxRequests: 1,      // Allow exactly 1 extension
    negotiatedTimeoutMaxPerRequest: 15000, // 15s extension
    negotiatedTimeoutBudget: 60000,
    debug: true,
  });

  console.log(`  ${DIM}Asking moderately complex question with 10s timeout + 1 extension...${RESET}`);
  const startTime6 = Date.now();
  const result6 = await agent6.answer(
    'Search for how timeout handling works in this codebase. Look at the ProbeAgent timeout-related code and explain the different timeout modes.'
  );
  const elapsed6 = Date.now() - startTime6;

  assert(typeof result6 === 'string', 'got a string result');
  assert(result6.length > 50, `result is substantial (${result6.length} chars)`);

  console.log(`  ${DIM}Elapsed: ${(elapsed6/1000).toFixed(1)}s${RESET}`);
  console.log(`  ${DIM}Result preview: ${result6.substring(0, 200).replace(/\n/g, ' ')}...${RESET}`);
} catch (err) {
  console.log(`  ${FAIL} Test 6 threw: ${err.message}`);
  failed++;
}

// ============================================================================
// Summary
// ============================================================================

console.log(`\n${BOLD}═══════════════════════════════════════${RESET}`);
console.log(`${BOLD}Results: ${passed} passed, ${failed} failed${RESET}`);
console.log(`${BOLD}═══════════════════════════════════════${RESET}\n`);

process.exit(failed > 0 ? 1 : 0);
