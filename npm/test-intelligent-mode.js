/**
 * Test the new intelligent 3-phase analyze_all mode
 */

import dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

dotenv.config({ path: join(__dirname, '..', '.env') });

import { analyzeAll } from './src/tools/analyzeAll.js';

const PROVIDER = 'google';
const MODEL = 'gemini-2.0-flash';

async function runTest(name, options) {
  console.log(`\n${'='.repeat(70)}`);
  console.log(`TEST: ${name}`);
  console.log('='.repeat(70));

  const startTime = Date.now();
  try {
    const result = await analyzeAll({
      provider: PROVIDER,
      model: MODEL,
      debug: true,
      ...options
    });
    const duration = ((Date.now() - startTime) / 1000).toFixed(1);

    console.log(`\n✓ COMPLETED in ${duration}s`);
    console.log('\n--- RESULT ---');
    console.log(result);
    console.log('--- END RESULT ---\n');
    return { success: true, duration, result };
  } catch (error) {
    const duration = ((Date.now() - startTime) / 1000).toFixed(1);
    console.error(`\n✗ FAILED after ${duration}s: ${error.message}`);
    return { success: false, duration, error: error.message };
  }
}

async function main() {
  console.log('╔══════════════════════════════════════════════════════════════════════╗');
  console.log('║         INTELLIGENT MODE TEST SUITE FOR analyze_all                  ║');
  console.log('╚══════════════════════════════════════════════════════════════════════╝');

  const results = [];

  // Test 1: Simple question about tools
  results.push(await runTest(
    '1. What tools are available?',
    {
      question: 'What are all the different tools available in this codebase?',
      path: './src/tools'
    }
  ));

  // Test 2: Question about patterns (searching in src directory for search-related patterns)
  results.push(await runTest(
    '2. Error handling patterns',
    {
      question: 'What error handling patterns are used in the search module (search.js)?',
      path: './src'
    }
  ));

  // Test 3: Counting question
  results.push(await runTest(
    '3. Count exported items',
    {
      question: 'How many functions and constants are exported from the tools directory?',
      path: './src/tools'
    }
  ));

  // Test 4: Categorization question (search in agent directory)
  results.push(await runTest(
    '4. Categorize dependencies',
    {
      question: 'What external dependencies does the ProbeAgent module use? Categorize them by purpose.',
      path: './src/agent'
    }
  ));

  // Summary
  console.log('\n' + '═'.repeat(70));
  console.log('SUMMARY');
  console.log('═'.repeat(70));

  const passed = results.filter(r => r.success).length;
  const failed = results.length - passed;

  console.log(`Total: ${results.length}`);
  console.log(`✓ Passed: ${passed}`);
  console.log(`✗ Failed: ${failed}`);
  console.log(`Success Rate: ${((passed / results.length) * 100).toFixed(0)}%`);

  // Timing summary
  console.log('\nTiming:');
  results.forEach((r, i) => {
    console.log(`  Test ${i + 1}: ${r.duration}s ${r.success ? '✓' : '✗'}`);
  });
}

main().catch(console.error);
