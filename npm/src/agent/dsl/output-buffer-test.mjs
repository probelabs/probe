#!/usr/bin/env node
/**
 * Quick E2E test of the output buffer feature.
 */

import { createDSLRuntime } from './runtime.js';

const outputBuffer = { items: [] };
const runtime = createDSLRuntime({
  toolImplementations: {
    search: { execute: async (p) => 'Result for: ' + p.query + '\nLine 1\nLine 2\nLine 3' },
  },
  llmCall: async (inst, data) => 'LLM processed: ' + String(data).substring(0, 50),
  outputBuffer,
});

let passed = 0;
let failed = 0;

function check(name, condition) {
  if (condition) {
    console.log('  ✓ ' + name);
    passed++;
  } else {
    console.log('  ✗ ' + name);
    failed++;
  }
}

// Test 1: output() writes to buffer, return value separate
console.log('\nTest 1: output() + return');
outputBuffer.items = [];
const r1 = await runtime.execute(`
  const data = search("test query");
  output("## Full Results");
  output(data);
  return "Summary: found results";
`, 'test 1');

check('status is success', r1.status === 'success');
check('return value correct', r1.result === 'Summary: found results');
check('buffer has 2 items', outputBuffer.items.length === 2);
check('buffer[0] is header', outputBuffer.items[0] === '## Full Results');
check('buffer[1] has search data', outputBuffer.items[1].includes('Result for: test query'));
check('logs include [output]', r1.logs.some(l => l.startsWith('[output]')));

// Test 2: output() with JSON object
console.log('\nTest 2: output() with JSON');
outputBuffer.items = [];
const r2 = await runtime.execute(`
  output({ customers: ["Acme", "BigCo"], count: 2 });
  return "Found 2 customers";
`, 'test 2');

check('status is success', r2.status === 'success');
check('return is summary', r2.result === 'Found 2 customers');
check('buffer has 1 item', outputBuffer.items.length === 1);
const parsed = JSON.parse(outputBuffer.items[0]);
check('parsed JSON correct', parsed.count === 2 && parsed.customers[0] === 'Acme');

// Test 3: output() persists across calls (accumulates)
console.log('\nTest 3: Accumulation across calls');
outputBuffer.items = [];
await runtime.execute(`output("first call")`, 'call 1');
await runtime.execute(`output("second call")`, 'call 2');
check('buffer has 2 items from 2 calls', outputBuffer.items.length === 2);
check('items correct', outputBuffer.items[0] === 'first call' && outputBuffer.items[1] === 'second call');

// Test 4: output() ignores null/undefined
console.log('\nTest 4: Ignores null/undefined');
outputBuffer.items = [];
const r4 = await runtime.execute(`
  output(null);
  output(undefined);
  output("real content");
  return "done";
`, 'test 4');
check('buffer has only 1 item', outputBuffer.items.length === 1);
check('only real content', outputBuffer.items[0] === 'real content');

// Test 5: Large table simulation
console.log('\nTest 5: Large table');
outputBuffer.items = [];
const r5 = await runtime.execute(`
  var rows = [];
  for (var i = 0; i < 100; i++) {
    rows.push("| Customer " + i + " | Tech | Active |");
  }
  var header = "| Customer | Industry | Status |\\n| --- | --- | --- |\\n";
  var table = header;
  for (const row of rows) {
    table = table + row + "\\n";
  }
  output(table);
  return "Generated table with 100 customers";
`, 'test 5');

check('status is success', r5.status === 'success');
check('return is summary', r5.result === 'Generated table with 100 customers');
check('buffer has table', outputBuffer.items[0].includes('Customer 99'));
check('table is large', outputBuffer.items[0].length > 2000);

// Test 6: No outputBuffer = no output() function
console.log('\nTest 6: No outputBuffer');
const runtimeNoBuffer = createDSLRuntime({
  toolImplementations: {
    search: { execute: async (p) => 'ok' },
  },
  llmCall: async () => 'ok',
});

const r6 = await runtimeNoBuffer.execute(`
  if (typeof output === "undefined") {
    return "output not available";
  }
  return "output available";
`, 'test 6');
check('output not available without buffer', r6.result === 'output not available');

// Summary
console.log('\n' + '═'.repeat(50));
console.log(`  Output Buffer E2E: ${passed} passed, ${failed} failed`);
console.log('═'.repeat(50));
process.exit(failed > 0 ? 1 : 0);
