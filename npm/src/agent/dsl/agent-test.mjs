#!/usr/bin/env node
/**
 * Agent-realistic test: the LLM writes DSL scripts itself.
 *
 * This simulates the real production flow:
 * 1. We give the LLM a task + the tool definition (system prompt)
 * 2. The LLM generates the DSL script
 * 3. The runtime validates, transforms, and executes it
 * 4. The result comes back
 *
 * Usage:
 *   node npm/src/agent/dsl/agent-test.mjs
 */

import { createDSLRuntime } from './runtime.js';
import { getExecutePlanToolDefinition } from '../../tools/executePlan.js';
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

// For generating DSL scripts (the "agent" role)
async function agentGenerate(systemPrompt, userTask) {
  const result = await generateText({
    model: google('gemini-2.5-flash'),
    system: systemPrompt,
    prompt: userTask,
    temperature: 0.3,
    maxTokens: 4000,
  });
  return result.text;
}

const cwd = projectRoot;

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
          cwd,
        });
      } catch (e) {
        return "Extract error: " + e.message;
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
        return "listFiles error: " + e.message;
      }
    },
  },
};

const runtime = createDSLRuntime({
  toolImplementations,
  llmCall,
  mapConcurrency: 3,
  timeoutMs: 60000,       // 60s timeout per execution
  maxLoopIterations: 5000, // loop guard
});

/**
 * Strip markdown fences and XML tags that LLMs sometimes wrap code in.
 */
function stripCodeWrapping(code) {
  let s = String(code || '');
  s = s.replace(/^```(?:javascript|js)?\n?/gm, '').replace(/```$/gm, '');
  s = s.replace(/<\/?(?:execute_plan|code)>/g, '');
  return s.trim();
}

// The tool definition that goes into the agent's system prompt
const toolDef = getExecutePlanToolDefinition(['search', 'extract', 'LLM', 'map', 'chunk', 'listFiles', 'log', 'range', 'flatten', 'unique', 'groupBy']);

const SYSTEM_PROMPT = `You are a coding assistant with access to the execute_plan tool.

${toolDef}

When the user asks a question that requires searching a codebase, batch processing, or handling large data,
write a DSL script to handle it. Return ONLY the JavaScript code — no markdown fences, no explanation,
no \`\`\` blocks. Just the raw code that goes into the execute_plan tool.

CRITICAL RULES:
- Do NOT use async/await — the runtime handles it.
- Do NOT use template literals (backticks) — use string concatenation with +.
- Do NOT use shorthand properties like { key } — use { key: key }.
- search() returns a STRING, not an array. Use chunk() to split it into an array.
- map(items, fn) requires an ARRAY as first argument. Do NOT pass a string to map().
- Do NOT use .map(), .forEach(), .filter(), .join() array methods. Use for..of loops or the global map() function.
- To join an array, use a for..of loop: var s = ""; for (const item of arr) { s = s + item + "\\n"; }
- Do NOT define helper functions that call tools. Write all logic inline or use for..of loops.
- Use String(value) to safely convert to string before calling .trim() or .split().
- Do NOT use regex literals (/pattern/) — use String methods like indexOf, includes, startsWith instead.
- ONLY call functions listed in the tool definition. Do NOT invent or guess function names.
- ALWAYS write executable DSL code, never answer in plain text.
- Always return a value at the end.`;

// ── Test runner ──
let testNum = 0;
let passed = 0;
let failed = 0;

const MAX_RETRIES = 2;

async function runAgentTest(taskDescription, check) {
  testNum++;
  console.log(`\n${'─'.repeat(70)}`);
  console.log(`▶ Test ${testNum}: ${taskDescription}`);

  const start = Date.now();

  try {
    // Step 1: Agent generates the DSL script
    console.log('  [1/4] Agent generating DSL script...');
    const generatedCode = await agentGenerate(SYSTEM_PROMPT, taskDescription);
    let currentCode = stripCodeWrapping(generatedCode);
    console.log(`  Generated (${currentCode.split('\n').length} lines):`);
    const preview = currentCode.split('\n').slice(0, 6).map(l => '    ' + l).join('\n');
    console.log(preview);
    if (currentCode.split('\n').length > 6) console.log('    ...');

    // Step 2: Execute with self-healing retries
    let result;
    let attempt = 0;

    while (attempt <= MAX_RETRIES) {
      console.log(`  [2/4] Executing DSL script${attempt > 0 ? ' (retry ' + attempt + ')' : ''}...`);
      result = await runtime.execute(currentCode, taskDescription);

      if (result.status === 'success') break;

      // Execution failed — try self-healing
      const logOutput = result.logs.length > 0 ? '\nLogs: ' + result.logs.join(' | ') : '';
      const errorMsg = result.error + logOutput;
      console.log(`  [!] Execution failed: ${errorMsg.substring(0, 150)}`);

      if (attempt >= MAX_RETRIES) break;

      console.log(`  [3/4] Self-healing — asking LLM to fix (attempt ${attempt + 1})...`);
      const fixPrompt = `The following DSL script failed with an error. Fix the script and return ONLY the corrected JavaScript code — no markdown, no explanation, no backtick fences.

ORIGINAL SCRIPT:
${currentCode}

ERROR:
${errorMsg}

RULES REMINDER:
- search(), listFiles(), extract() all return STRINGS, not arrays.
- Use chunk(stringData) to split a string into an array of chunks.
- map(items, fn) requires an ARRAY as first argument. Do NOT pass strings to map().
- Do NOT use .map(), .forEach(), .filter(), .join() — use for..of loops instead.
- Do NOT define helper functions that call tools — write logic inline.
- Do NOT use async/await, template literals, or shorthand properties.
- Do NOT use regex literals (/pattern/) — use String methods like indexOf, includes, startsWith instead.
- String concatenation with +, not template literals.`;

      const fixedCode = await llmCall(fixPrompt, '', { maxTokens: 4000, temperature: 0.2 });
      currentCode = stripCodeWrapping(fixedCode);

      if (!currentCode) {
        console.log('  [!] Self-heal returned empty code');
        break;
      }

      console.log(`  Fixed code (${currentCode.split('\n').length} lines):`);
      const fixPreview = currentCode.split('\n').slice(0, 4).map(l => '    ' + l).join('\n');
      console.log(fixPreview);
      if (currentCode.split('\n').length > 4) console.log('    ...');

      attempt++;
    }

    const elapsed = Date.now() - start;
    console.log(`  [4/4] Checking result... (${elapsed}ms)`);

    if (result.status === 'error') {
      console.log(`  ✗ EXECUTION ERROR after ${attempt} retries (${elapsed}ms)`);
      console.log(`  Error: ${result.error.substring(0, 200)}`);
      if (result.logs.length) console.log(`  Logs: ${result.logs.join(' | ')}`);
      failed++;
      return;
    }

    const checkResult = check(result);
    if (checkResult === true || checkResult === undefined) {
      const healNote = attempt > 0 ? ` (self-healed after ${attempt} ${attempt === 1 ? 'retry' : 'retries'})` : '';
      console.log(`  ✓ PASSED${healNote} (${elapsed}ms)`);
      const resultPreview = typeof result.result === 'string'
        ? result.result.substring(0, 300)
        : JSON.stringify(result.result, null, 2).substring(0, 300);
      console.log(`  Result: ${resultPreview}${resultPreview.length >= 300 ? '...' : ''}`);
      if (result.logs && result.logs.filter(l => !l.startsWith('[runtime]')).length) {
        console.log(`  Logs: ${result.logs.filter(l => !l.startsWith('[runtime]')).join(' | ')}`);
      }
      passed++;
    } else {
      console.log(`  ✗ CHECK FAILED (${elapsed}ms) — ${checkResult}`);
      failed++;
    }
  } catch (e) {
    console.log(`  ✗ CRASHED — ${e.message}`);
    failed++;
  }
}

// ── Agent tests ──
async function main() {
  console.log('═'.repeat(70));
  console.log('  Agent-Realistic DSL Tests — LLM writes its own scripts');
  console.log('═'.repeat(70));

  // Test 1: Simple search + summarize
  await runAgentTest(
    'Search this codebase for how error handling is done and give me a brief summary.',
    (r) => {
      if (typeof r.result !== 'string') return 'Expected string result';
      if (r.result.length < 50) return 'Summary too short';
      return true;
    }
  );

  // Test 2: Find and count patterns
  await runAgentTest(
    'Write a DSL script to search this codebase for tool definitions (search, extract, query, etc.). Count how many unique tools are defined and return an object with the count and an array of tool names.',
    (r) => {
      if (!r.result) return 'No result';
      return true;
    }
  );

  // Test 3: Multi-file analysis
  await runAgentTest(
    'Look at the files in npm/src/agent/dsl/ directory — search for each one, and for each file give me a one-sentence description of what it does. Return as a list.',
    (r) => {
      if (!r.result) return 'No result';
      const s = typeof r.result === 'string' ? r.result : JSON.stringify(r.result);
      if (s.length < 50) return 'Result too short';
      return true;
    }
  );

  // Test 4: Code quality check
  await runAgentTest(
    'Search for all TODO and FIXME comments in this codebase. Group them by urgency (TODO vs FIXME) and summarize what needs attention.',
    (r) => {
      if (!r.result) return 'No result';
      return true;
    }
  );

  // Test 5: Complex analysis requiring chunking
  await runAgentTest(
    'Analyze the test coverage of this project. Search for test files, see what modules they test, and identify any modules that might be missing tests. Give me a brief report.',
    (r) => {
      if (!r.result) return 'No result';
      const s = typeof r.result === 'string' ? r.result : JSON.stringify(r.result);
      if (s.length < 50) return 'Report too short';
      return true;
    }
  );

  // Test 6: Data extraction + classification
  await runAgentTest(
    'Find all the Zod schemas defined in this codebase (search for "z.object"). For each schema, extract its name and list its fields. Return a structured summary.',
    (r) => {
      if (!r.result) return 'No result';
      return true;
    }
  );

  // ── Summary ──
  console.log(`\n${'═'.repeat(70)}`);
  console.log(`  Agent-Realistic Results: ${passed} passed, ${failed} failed, ${testNum} total`);
  console.log('═'.repeat(70));

  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => {
  console.error('Fatal error:', e);
  process.exit(1);
});
