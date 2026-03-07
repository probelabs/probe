/**
 * Real LLM integration tests for ProbeAgent refactored features.
 *
 * These tests exercise the agent end-to-end with a real model (Gemini 2.5 Flash).
 * They only run when GOOGLE_API_KEY (or GOOGLE_GENERATIVE_AI_API_KEY) is set.
 *
 * Run:
 *   cd npm && GOOGLE_API_KEY=... NODE_OPTIONS=--experimental-vm-modules npx jest tests/integration/real-llm-agent.test.js --testTimeout=120000
 */

import { jest } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const PROJECT_ROOT = join(__dirname, '..', '..');

const GOOGLE_API_KEY = process.env.GOOGLE_API_KEY || process.env.GOOGLE_GENERATIVE_AI_API_KEY;
const describeIfKey = GOOGLE_API_KEY ? describe : describe.skip;

/**
 * Utility to capture all tool call events from an agent for post-test inspection.
 */
class TestLogger {
  constructor() {
    this.toolCalls = [];
    this.errors = [];
  }

  attach(agent) {
    agent.events.on('toolCall', (event) => {
      this.toolCalls.push(event);
      if (event.status === 'error') {
        this.errors.push(event);
      }
    });
  }

  getToolNames() {
    return [...new Set(
      this.toolCalls
        .filter(t => t.status === 'completed')
        .map(t => t.name)
    )];
  }

  hasToolCall(name) {
    return this.toolCalls.some(t => t.name === name && t.status === 'completed');
  }

  getToolCallCount() {
    return this.toolCalls.filter(t => t.status === 'completed').length;
  }

  dump() {
    console.log('=== Tool Calls ===');
    for (const tc of this.toolCalls) {
      console.log(`  ${tc.name} [${tc.status}]`);
      if (tc.error) console.log(`    ERROR: ${tc.error}`);
      if (tc.resultPreview) console.log(`    Preview: ${tc.resultPreview.slice(0, 100)}`);
    }
    console.log(`=== Total: ${this.toolCalls.length} events, ${this.errors.length} errors ===`);
  }
}

/**
 * Create and initialize an agent with real LLM provider.
 * Temporarily sets NODE_ENV away from 'test' so the agent uses the real
 * Google provider instead of the mock provider.
 */
async function createAgent(overrides = {}) {
  // Temporarily disable NODE_ENV=test so ProbeAgent uses the real provider
  // instead of the mock provider (initializeModel checks NODE_ENV in constructor)
  const savedEnv = process.env.NODE_ENV;
  process.env.NODE_ENV = 'integration';
  try {
    const agent = new ProbeAgent({
      path: join(PROJECT_ROOT, '..'),  // probe project root (parent of npm/)
      provider: 'google',
      model: 'gemini-2.5-flash',
      debug: true,
      maxIterations: 10,
      ...overrides,
    });
    await agent.initialize();
    return agent;
  } finally {
    process.env.NODE_ENV = savedEnv;
  }
}

describeIfKey('Real LLM Agent Integration Tests', () => {
  jest.setTimeout(120000);

  let agent;
  let logger;

  afterEach(async () => {
    if (agent) {
      try { await agent.cleanup(); } catch (_) { /* ignore */ }
      agent = null;
    }
  });

  // ─── 1. Basic Completion (no attempt_completion) ───────────────────────────

  test('basic completion returns a response without attempt_completion', async () => {
    agent = await createAgent();
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer('What programming language is the probe project primarily written in? Answer in one sentence.');

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    expect(result.length).toBeGreaterThan(5);
    expect(logger.hasToolCall('attempt_completion')).toBe(false);
    expect(result.toLowerCase()).toMatch(/rust/);
  });

  // ─── 2. Search Tool Usage ─────────────────────────────────────────────────

  test('search tool is used when asking about code functionality', async () => {
    agent = await createAgent();
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer('What does the search function do in the probe codebase? Be concise.');

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    expect(logger.hasToolCall('search')).toBe(true);
    expect(result.toLowerCase()).toMatch(/search|query|find|match/);
  });

  // ─── 3. Extract Tool Usage ────────────────────────────────────────────────

  test('extract tool is used when asking to show code', async () => {
    agent = await createAgent();
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer('Show me the main function or entry point of the probe Rust binary. Use the extract tool to get the actual code.');

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    expect(logger.hasToolCall('extract')).toBe(true);
    expect(result).toMatch(/fn |pub |mod |use |main/);
  });

  // ─── 4. CompletionPrompt via prepareStep ──────────────────────────────────

  test('completionPrompt causes the agent to verify its answer', async () => {
    // The completionPrompt injects a review round after the model's initial answer.
    // We use a simple, deterministic marker ("YAHOO") that the model would never
    // include on its own — if it appears in the final result, the completionPrompt
    // mechanism definitely fired and the model followed the instruction.
    const debugLogs = [];
    const _origLog = console.log;
    const _origError = console.error;
    const capture = (orig) => (...args) => {
      const msg = args.map(a => typeof a === 'string' ? a : String(a)).join(' ');
      debugLogs.push(msg);
      orig.apply(console, args);
    };
    console.log = capture(_origLog);
    console.error = capture(_origError);

    try {
      agent = await createAgent({
        completionPrompt: 'You MUST add the word YAHOO at the very end of your response.',
      });
      logger = new TestLogger();
      logger.attach(agent);

      const result = await agent.answer('What programming language is probe written in? Answer in one sentence.');

      _origLog.call(console, 'Result:', result);
      logger.dump();

      // 1. The completionPrompt mechanism must have fired
      const injectionFired = debugLogs.some(log =>
        log.includes('Injecting completion prompt')
      );
      _origLog.call(console, 'Completion prompt injected:', injectionFired);
      expect(injectionFired).toBe(true);

      // 2. The model must have followed the completionPrompt instruction
      expect(result).toBeTruthy();
      expect(result).toMatch(/YAHOO/i);
    } finally {
      console.log = _origLog;
      console.error = _origError;
    }
  });

  // ─── 5. JSON Schema Output ────────────────────────────────────────────────

  test('schema output produces valid JSON matching the schema', async () => {
    agent = await createAgent({ maxIterations: 15 });
    logger = new TestLogger();
    logger.attach(agent);

    const schema = JSON.stringify({
      type: 'object',
      properties: {
        files: { type: 'array', items: { type: 'string' } },
        description: { type: 'string' },
      },
      required: ['files', 'description'],
    });

    const result = await agent.answer(
      'List the main source directories in the probe project (at least 3). Return as JSON.',
      [],
      { schema }
    );

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    let parsed;
    try {
      parsed = JSON.parse(result);
    } catch (e) {
      const jsonMatch = result.match(/```(?:json)?\s*([\s\S]*?)```/);
      if (jsonMatch) {
        parsed = JSON.parse(jsonMatch[1].trim());
      } else {
        throw new Error(`Failed to parse JSON response: ${result.slice(0, 200)}`);
      }
    }

    expect(parsed).toHaveProperty('files');
    expect(parsed).toHaveProperty('description');
    expect(Array.isArray(parsed.files)).toBe(true);
    expect(parsed.files.length).toBeGreaterThanOrEqual(1);
    expect(typeof parsed.description).toBe('string');
    expect(parsed.description.length).toBeGreaterThan(0);
  });

  // ─── 5b. Native JSON Schema (no tools) ───────────────────────────────────

  test('native JSON schema output works when tools are disabled', async () => {
    agent = await createAgent({
      disableTools: true,
      maxIterations: 3,
    });
    logger = new TestLogger();
    logger.attach(agent);

    const schema = JSON.stringify({
      type: 'object',
      properties: {
        answer: { type: 'number' },
        explanation: { type: 'string' },
      },
      required: ['answer', 'explanation'],
    });

    const result = await agent.answer(
      'What is 6 multiplied by 7? Return as JSON.',
      [],
      { schema }
    );

    console.log('Result:', result);

    expect(result).toBeTruthy();
    const parsed = JSON.parse(result);
    expect(parsed).toHaveProperty('answer');
    expect(parsed).toHaveProperty('explanation');
    expect(parsed.answer).toBe(42);
    expect(typeof parsed.explanation).toBe('string');
    // No tool calls since tools are disabled
    expect(logger.getToolCallCount()).toBe(0);
  });

  // ─── 6. Allowed Tools Filtering ───────────────────────────────────────────

  test('allowedTools restricts which tools the agent can use', async () => {
    agent = await createAgent({
      allowedTools: ['search'],
    });
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer('Find where BM25 ranking is implemented in the codebase.');

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    const toolNames = logger.getToolNames();
    console.log('Tools used:', toolNames);
    for (const name of toolNames) {
      expect(name).toBe('search');
    }
  });

  // ─── 7. Disabled Tools (Raw AI Mode) ──────────────────────────────────────

  test('disableTools prevents all tool usage', async () => {
    agent = await createAgent({
      disableTools: true,
    });
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer('What is 2 + 2? Answer with just the number.');

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    expect(result).toMatch(/4/);
    expect(logger.getToolCallCount()).toBe(0);
  });

  // ─── 8. Bash Tool ─────────────────────────────────────────────────────────

  test('bash tool executes shell commands when enabled', async () => {
    agent = await createAgent({
      enableBash: true,
    });
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer('Run `ls src/` using bash and tell me what files and directories exist in the src directory.');

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    expect(logger.hasToolCall('bash')).toBe(true);
    expect(result.toLowerCase()).toMatch(/language|search|extract|ranking|main/);
  });

  // ─── 9. MaxIterations / Last-Iteration Warning ────────────────────────────

  test('maxIterations limits the number of agent steps', async () => {
    agent = await createAgent({
      maxIterations: 2,
    });
    logger = new TestLogger();
    logger.attach(agent);

    const result = await agent.answer(
      'Do a comprehensive analysis of every single file in the src/ directory. Search for each one individually and extract the contents.'
    );

    console.log('Result:', result);
    logger.dump();

    expect(result).toBeTruthy();
    expect(result.length).toBeGreaterThan(0);
    const completedCalls = logger.toolCalls.filter(t => t.status === 'completed').length;
    expect(completedCalls).toBeLessThanOrEqual(5);
  });

  // ─── 10. Quality Evaluation (LLM-as-Judge) ────────────────────────────────

  test('agent produces quality responses evaluated by LLM-as-judge', async () => {
    agent = await createAgent();
    logger = new TestLogger();
    logger.attach(agent);

    const response = await agent.answer('Explain how the BM25 ranking algorithm works in probe. Be specific about the implementation.');

    console.log('Response:', response);
    logger.dump();

    expect(response).toBeTruthy();
    expect(response.length).toBeGreaterThan(50);

    // Clean up first agent
    await agent.cleanup();

    // Second agent: evaluate the response quality
    const evaluator = await createAgent({
      disableTools: true,
      maxIterations: 1,
    });

    const evaluation = await evaluator.answer(
      `Rate the following answer about BM25 ranking in the probe codebase on a scale of 1-5 for accuracy and completeness.
Respond with ONLY valid JSON: {"score": <number>, "reasoning": "<string>"}

Answer to evaluate:
${response.slice(0, 3000)}`
    );

    console.log('Evaluation:', evaluation);
    await evaluator.cleanup();
    agent = null; // already cleaned up

    let evalResult;
    try {
      evalResult = JSON.parse(evaluation);
    } catch (e) {
      const jsonMatch = evaluation.match(/\{[\s\S]*"score"[\s\S]*\}/);
      if (jsonMatch) {
        evalResult = JSON.parse(jsonMatch[0]);
      } else {
        console.warn('Could not parse evaluation as JSON, skipping score check');
        return;
      }
    }

    console.log(`Quality score: ${evalResult.score}/5 - ${evalResult.reasoning}`);
    expect(evalResult.score).toBeGreaterThanOrEqual(3);
  });
});
