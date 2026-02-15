#!/usr/bin/env node
/**
 * Direct DSL runtime test against customer-insights repo.
 * Bypasses ProbeAgent — runs scripts directly against the runtime.
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
if (!apiKey) { console.error('No API key'); process.exit(1); }

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

const TARGET = '/tmp/customer-insights';

const runtime = createDSLRuntime({
  toolImplementations: {
    search: { execute: async (params) => {
      try {
        return await search({ query: params.query, path: TARGET, maxTokens: 20000, timeout: 60 });
      } catch(e) { return 'Search error: ' + e.message; }
    }},
    extract: { execute: async (params) => {
      try {
        return await extract({ targets: params.targets, cwd: TARGET });
      } catch(e) { return 'Extract error: ' + e.message; }
    }},
    listFiles: { execute: async (params) => {
      try {
        return await search({ query: params.pattern || 'customer', path: TARGET, filesOnly: true, maxTokens: 10000, timeout: 60 });
      } catch(e) { return 'listFiles error: ' + e.message; }
    }},
  },
  llmCall,
  mapConcurrency: 3,
  timeoutMs: 300000,
  maxLoopIterations: 5000,
});

console.log('═'.repeat(70));
console.log('  Direct DSL Pipeline Test — customer-insights repo');
console.log('═'.repeat(70));

const start = Date.now();
const result = await runtime.execute(`
// Step 1: Broad search for customer data
const results = search("customer onboarding playbook");
log("Search returned " + String(results).length + " chars");

// Step 2: Split into chunks and extract customer info using LLM
const chunks = chunk(results);
log("Split into " + chunks.length + " chunks");

const classified = map(chunks, (c) => LLM(
  "Extract customer names and their industry from this text. " +
  "Return a JSON array: [{customer: string, industry: string, notes: string}]. " +
  "Return ONLY valid JSON array, no other text.",
  c
));

// Step 3: Accumulate parsed results
var allCustomers = [];
for (const batch of classified) {
  try {
    var text = String(batch).trim();
    var jsonStart = text.indexOf("[");
    var jsonEnd = text.lastIndexOf("]");
    if (jsonStart >= 0 && jsonEnd > jsonStart) {
      text = text.substring(jsonStart, jsonEnd + 1);
    }
    var parsed = JSON.parse(text);
    if (Array.isArray(parsed)) {
      for (const item of parsed) { allCustomers.push(item); }
    }
  } catch (e) {
    log("Parse error, skipping chunk");
  }
}

log("Total customers extracted: " + allCustomers.length);

// Step 4: Deduplicate
var seen = {};
var uniqueCustomers = [];
for (const c of allCustomers) {
  var key = String(c.customer || "").trim().toLowerCase();
  if (key.length > 0 && !seen[key]) {
    seen[key] = true;
    uniqueCustomers.push(c);
  }
}

log("Unique customers: " + uniqueCustomers.length);

// Step 5: Build markdown table
var table = "| Customer | Industry | Notes |\\n|---|---|---|\\n";
for (const c of uniqueCustomers) {
  table = table + "| " + (c.customer || "Unknown") + " | " + (c.industry || "Unknown") + " | " + (c.notes || "-") + " |\\n";
}

// Step 6: Small LLM summary
const summary = LLM(
  "Based on this customer table, write a brief 2-3 sentence summary of the customer base — what industries are represented, any patterns.",
  table
);

return table + "\\n" + summary;
`, 'Customer classification pipeline');

const elapsed = Math.round((Date.now() - start) / 1000);

console.log('\n' + '─'.repeat(70));
console.log(`Status: ${result.status} (${elapsed}s)`);
console.log(`Logs: ${result.logs.join(' | ')}`);

if (result.status === 'error') {
  console.log(`Error: ${result.error}`);
} else {
  console.log('─'.repeat(70));
  console.log(result.result);
}

process.exit(result.status === 'error' ? 1 : 0);
