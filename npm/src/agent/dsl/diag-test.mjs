#!/usr/bin/env node
/**
 * Diagnostic test — traces exactly what execute_plan returns through ProbeAgent.
 */

import { ProbeAgent } from '../ProbeAgent.js';
import { config } from 'dotenv';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '../../../..');

config({ path: resolve(projectRoot, '.env') });

const apiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
if (!apiKey) {
  console.error('ERROR: No Google API key found');
  process.exit(1);
}

const agent = new ProbeAgent({
  path: '/tmp/customer-insights',
  provider: 'google',
  model: 'gemini-2.5-flash',
  enableExecutePlan: true,
  maxIterations: 15,
});

let callNum = 0;

agent.events.on('toolCall', (event) => {
  if (event.status === 'started') {
    if (event.name === 'execute_plan') {
      callNum++;
      console.log(`\n>>> EXECUTE_PLAN #${callNum} START`);
      console.log(`>>> CODE:\n${String(event.args?.code || '').substring(0, 1200)}`);
      if (String(event.args?.code || '').length > 1200) console.log('>>> ... (truncated)');
    }
  }
  if (event.status === 'error') {
    console.log(`>>> TOOL ERROR: ${event.name}: ${event.error}`);
  }
});

await agent.initialize();

// Monkey-patch to see full results
const origExecute = agent.toolImplementations.execute_plan.execute;
agent.toolImplementations.execute_plan.execute = async (params) => {
  const result = await origExecute(params);
  const resultStr = typeof result === 'string' ? result : JSON.stringify(result);
  console.log(`\n>>> EXECUTE_PLAN #${callNum} RETURNED (${resultStr.length} chars):`);
  console.log(`>>> ${resultStr.substring(0, 500)}`);
  if (resultStr.length > 500) console.log(`>>> ... (${resultStr.length - 500} more chars)`);
  return result;
};

const query = 'Analyze ALL customer files in this repository. For every customer, classify them by industry. Produce a markdown table with columns: Customer, Industry, Use Case.';

console.log(`\nQUERY: ${query}\n`);

try {
  const result = await Promise.race([
    agent.answer(query),
    new Promise((_, reject) => setTimeout(() => reject(new Error('Timeout 600s')), 600000)),
  ]);

  console.log(`\n${'='.repeat(60)}`);
  console.log(`FINAL RESULT (${String(result).length} chars):`);
  console.log(String(result).substring(0, 2000));
  console.log(`${'='.repeat(60)}`);
} catch (e) {
  console.log(`\nFAILED: ${e.message}`);
}

try { await agent.close(); } catch (e) {}
process.exit(0);
