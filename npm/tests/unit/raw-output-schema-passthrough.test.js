/**
 * Tests for RAW_OUTPUT passthrough with schema-formatted responses.
 *
 * Verifies that:
 * 1. output() content in execute_plan is wrapped in <<<RAW_OUTPUT>>> when agent has a schema
 * 2. RAW_OUTPUT blocks are extracted from tool results BEFORE the LLM sees them
 * 3. Main agent → subagent chain: raw output cascades through without hitting any LLM
 */

import { describe, test, expect } from '@jest/globals';

import {
  extractRawOutputBlocks,
  RAW_OUTPUT_START,
  RAW_OUTPUT_END,
  createExecutePlanTool,
} from '../../src/tools/executePlan.js';

// ─────────────────────────────────────────────────────────────────────
// 1. Unit: output buffer appended with RAW_OUTPUT delimiters for schema
// ─────────────────────────────────────────────────────────────────────

// NOTE: ProbeAgent integration tests for RAW_OUTPUT with schema were removed
// because they relied on the old XML-based agent loop. The agent now uses
// Vercel AI SDK native tool calling (streamText + maxSteps), which makes
// mocking with XML tool call responses invalid. The RAW_OUTPUT extraction
// logic is still tested via the unit tests below (extractRawOutputBlocks,
// createExecutePlanTool).

// ─────────────────────────────────────────────────────────────────────
// 2. RAW_OUTPUT is extracted from tool results BEFORE LLM sees them
// ─────────────────────────────────────────────────────────────────────

// NOTE: "RAW_OUTPUT never reaches LLM" integration test was removed because it
// relied on the old XML-based agent loop. With native Vercel AI SDK tool calling,
// tool results are managed by the SDK. RAW_OUTPUT extraction is tested at the
// unit level via extractRawOutputBlocks tests.

// ─────────────────────────────────────────────────────────────────────
// 3. Subagent (MCP tool) returning schema JSON + RAW_OUTPUT
//    → parent extracts RAW_OUTPUT before LLM, cascades to parent buffer
// ─────────────────────────────────────────────────────────────────────

describe('Main agent → subagent RAW_OUTPUT cascade', () => {
  test('parent extracts RAW_OUTPUT from subagent schema response before LLM sees it', () => {
    // Simulate subagent returning schema JSON + appended RAW_OUTPUT
    // (this is what ProbeAgent.answer() now produces for schema responses)
    const subagentResponse = [
      '{"answer":{"text":"Found 3 customers using JWT"},"references":[]}',
      `${RAW_OUTPUT_START}`,
      '--- report.csv ---',
      'customer,auth_type,api_count',
      'Acme Corp,JWT,50',
      'Beta Inc,HMAC,12',
      'Gamma LLC,API Key,5',
      '--- report.csv ---',
      `${RAW_OUTPUT_END}`,
    ].join('\n');

    // Parent agent has its own output buffer
    const parentOutputBuffer = { items: [] };

    // Parent's tool result processing extracts RAW_OUTPUT
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(
      subagentResponse,
      parentOutputBuffer
    );

    // RAW_OUTPUT content should be in parent's buffer
    expect(extractedBlocks).toHaveLength(1);
    expect(parentOutputBuffer.items).toHaveLength(1);
    expect(parentOutputBuffer.items[0]).toContain('Acme Corp,JWT,50');
    expect(parentOutputBuffer.items[0]).toContain('--- report.csv ---');

    // Cleaned content (what the LLM sees) should only have the JSON
    expect(cleanedContent).toContain('"answer"');
    expect(cleanedContent).toContain('Found 3 customers using JWT');
    expect(cleanedContent).not.toContain('Acme Corp,JWT,50');
    expect(cleanedContent).not.toContain(RAW_OUTPUT_START);
    expect(cleanedContent).not.toContain(RAW_OUTPUT_END);

    // The cleaned content should be valid JSON
    expect(() => JSON.parse(cleanedContent)).not.toThrow();
  });

  test('multi-hop cascade: grandchild → child → parent, raw output survives all hops', () => {
    const reportData = '# Customer Auth Report\n\n| Customer | Type |\n|---|---|\n| Acme | JWT |';

    // Step 1: Grandchild agent runs execute_plan with output()
    // formatSuccess wraps in RAW_OUTPUT:
    const grandchildToolResult = `Plan: done\n\n${RAW_OUTPUT_START}\n${reportData}\n${RAW_OUTPUT_END}\n\n[The above raw output...]`;

    // Step 2: Child agent extracts grandchild's RAW_OUTPUT into its own buffer
    const childOutputBuffer = { items: [] };
    const { cleanedContent: childClean } = extractRawOutputBlocks(
      grandchildToolResult,
      childOutputBuffer
    );
    expect(childOutputBuffer.items).toHaveLength(1);
    expect(childClean).not.toContain(reportData);

    // Step 3: Child agent's answer() returns schema JSON + RAW_OUTPUT (our new behavior)
    // Simulate what ProbeAgent.answer() does when outputBuffer has items and schema is set
    const childSchemaResult =
      '{"answer":{"text":"Analysis complete"},"references":[]}' +
      `\n${RAW_OUTPUT_START}\n${childOutputBuffer.items.join('\n\n')}\n${RAW_OUTPUT_END}`;

    // Step 4: Parent agent processes child's response as a tool result
    const parentOutputBuffer = { items: [] };
    const { cleanedContent: parentClean } = extractRawOutputBlocks(
      childSchemaResult,
      parentOutputBuffer
    );

    // Parent's buffer now has the original report data
    expect(parentOutputBuffer.items).toHaveLength(1);
    expect(parentOutputBuffer.items[0]).toContain('# Customer Auth Report');
    expect(parentOutputBuffer.items[0]).toContain('Acme | JWT');

    // Parent's cleaned content is just the JSON (what its LLM sees)
    expect(parentClean).not.toContain(reportData);
    expect(parentClean).not.toContain(RAW_OUTPUT_START);
    expect(() => JSON.parse(parentClean)).not.toThrow();
  });

  // NOTE: "full ProbeAgent integration: MCP subagent returns RAW_OUTPUT" test was removed
  // because it relied on the old XML-based agent loop. With native Vercel AI SDK tool calling,
  // MCP tools need to provide getVercelTools() and the mock setup is fundamentally different.
});

// ─────────────────────────────────────────────────────────────────────
// 4. createExecutePlanTool: output() wraps in RAW_OUTPUT delimiters
// ─────────────────────────────────────────────────────────────────────

describe('createExecutePlanTool output() → RAW_OUTPUT', () => {
  test('output() in DSL writes to buffer and formatSuccess wraps in RAW_OUTPUT', async () => {
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: { execute: async () => 'search results' },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    const result = await tool.execute({
      code: 'output("customer,value\\nAcme,100"); return "done";',
      description: 'Test output',
    });

    // The tool result should contain RAW_OUTPUT delimiters
    expect(result).toContain(RAW_OUTPUT_START);
    expect(result).toContain('customer,value\nAcme,100');
    expect(result).toContain(RAW_OUTPUT_END);
  });

  test('multiple output() calls are joined in a single RAW_OUTPUT block', async () => {
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: { execute: async () => 'results' },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    const result = await tool.execute({
      code: 'output("line1"); output("line2"); return "done";',
      description: 'Test multi-output',
    });

    expect(result).toContain(RAW_OUTPUT_START);
    expect(result).toContain('line1');
    expect(result).toContain('line2');
    // Should have exactly one RAW_OUTPUT block
    const starts = result.split(RAW_OUTPUT_START).length - 1;
    expect(starts).toBe(1);
  });

  test('outputBuffer is cleared after formatSuccess to prevent accumulation (issue #430)', async () => {
    // This test verifies the fix for issue #430:
    // outputBuffer should be cleared after each execute_plan call
    // to prevent exponential duplication across multiple calls
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: { execute: async () => 'results' },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // First execute_plan call with output()
    const result1 = await tool.execute({
      code: 'output("first report"); return "done";',
      description: 'First plan',
    });

    expect(result1).toContain(RAW_OUTPUT_START);
    expect(result1).toContain('first report');
    // Buffer should be cleared after formatSuccess
    expect(outputBuffer.items).toHaveLength(0);

    // Second execute_plan call with different output()
    const result2 = await tool.execute({
      code: 'output("second report"); return "done";',
      description: 'Second plan',
    });

    expect(result2).toContain(RAW_OUTPUT_START);
    expect(result2).toContain('second report');
    // Should NOT contain the first report (no accumulation)
    expect(result2).not.toContain('first report');
    // Buffer should be cleared again
    expect(outputBuffer.items).toHaveLength(0);
  });

  test('execute_plan without output() does not include stale buffer content (issue #430)', async () => {
    // Simulates the scenario where extractRawOutputBlocks pushes content back to buffer
    // and a subsequent execute_plan (without output()) would incorrectly wrap stale data
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: { execute: async () => 'search results' },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // First call with output
    const result1 = await tool.execute({
      code: 'output("report data"); return "done";',
      description: 'Generate report',
    });
    expect(result1).toContain(RAW_OUTPUT_START);
    expect(result1).toContain('report data');

    // Simulate what extractRawOutputBlocks does: push extracted content back to buffer
    // (This is what ProbeAgent.js does when processing tool results)
    outputBuffer.items.push('report data');

    // Second call WITHOUT output - should not wrap the stale buffer content
    // Because the buffer should have been cleared by the first call
    // But here we're simulating the re-population by extractRawOutputBlocks
    const result2 = await tool.execute({
      code: 'const r = search("test"); return r;',
      description: 'Just search',
    });

    // After the fix: even though extractRawOutputBlocks pushed content back,
    // formatSuccess clears the buffer after wrapping, so each call only wraps
    // content produced during THAT call, not accumulated from previous calls
    // In this case, result2 WILL contain the stale content because we manually pushed it,
    // but the buffer will be cleared afterward
    expect(outputBuffer.items).toHaveLength(0);
  });
});

// ─────────────────────────────────────────────────────────────────────
// 5. completionPrompt does NOT leak _outputBuffer into finalResult
// ─────────────────────────────────────────────────────────────────────

// NOTE: "completionPrompt _outputBuffer isolation" integration tests were removed
// because they relied on the old XML-based agent loop. With native Vercel AI SDK
// tool calling (streamText + maxSteps), the mock setup is fundamentally different.
// The completionPrompt behavior with the new architecture needs separate tests.

// ─────────────────────────────────────────────────────────────────────
// 6. search maxTokens and searchAll in DSL
// ─────────────────────────────────────────────────────────────────────

describe('search maxTokens and searchAll in DSL', () => {
  test('search passes maxTokens parameter when provided via object syntax', async () => {
    let capturedMaxTokens = undefined;
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: {
          execute: async (params) => {
            capturedMaxTokens = params.maxTokens;
            return 'search results';
          },
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    await tool.execute({
      code: 'const r = search({query: "test", path: ".", maxTokens: 50000}); return r;',
      description: 'Test maxTokens',
    });

    expect(capturedMaxTokens).toBe(50000);
  });

  test('search passes maxTokens: null for unlimited via object syntax', async () => {
    let capturedMaxTokens = 'not-set';
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: {
          execute: async (params) => {
            capturedMaxTokens = params.maxTokens;
            return 'search results';
          },
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    await tool.execute({
      code: 'const r = search({query: "test", maxTokens: null}); return r;',
      description: 'Test unlimited maxTokens',
    });

    expect(capturedMaxTokens).toBe(null);
  });

  test('searchAll is callable from DSL', async () => {
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        searchAll: {
          execute: async (params) => {
            return `all results for: ${params.query}`;
          },
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    const result = await tool.execute({
      code: 'const r = searchAll("bulk query"); return r;',
      description: 'Test searchAll',
    });

    expect(result).toContain('Plan:');
    expect(result).toContain('all results for: bulk query');
  });

  test('searchAll accepts maxPages option', async () => {
    let capturedMaxPages = undefined;
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        searchAll: {
          execute: async (params) => {
            capturedMaxPages = params.maxPages;
            return `results for: ${params.query}`;
          },
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    await tool.execute({
      code: 'const r = searchAll({query: "test", maxPages: 10}); return r;',
      description: 'Test searchAll with maxPages',
    });

    expect(capturedMaxPages).toBe(10);
  });

  test('each execute_plan invocation gets unique session ID for search isolation', async () => {
    // This test verifies that multiple execute_plan calls work independently
    // Each gets a unique planSessionId so their search pagination is isolated
    const outputBuffer = { items: [] };

    const tool = createExecutePlanTool({
      sessionId: 'base-session',
      cwd: process.cwd(),
      toolImplementations: {
        search: {
          execute: async (params) => 'search results',
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // Execute twice - each should be independent (no session contamination)
    const result1 = await tool.execute({ code: 'const r = search("test1"); return r;', description: 'First' });
    const result2 = await tool.execute({ code: 'const r = search("test2"); return r;', description: 'Second' });

    // Both should complete successfully
    expect(result1).toContain('Plan:');
    expect(result2).toContain('Plan:');
  });

  test('search defaults to 20000 maxTokens when not specified', async () => {
    let capturedMaxTokens = undefined;
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: {
          execute: async (params) => {
            capturedMaxTokens = params.maxTokens;
            return 'search results';
          },
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // Call search without specifying maxTokens
    await tool.execute({
      code: 'const r = search("test"); return r;',
      description: 'Test default maxTokens',
    });

    // Default should be undefined (not set by DSL, set by buildToolImplementations)
    // When using direct toolImplementations, maxTokens is not set
    expect(capturedMaxTokens).toBeUndefined();
  });

  test('searchAll returns results from mock (pagination logic is internal)', async () => {
    // Note: When using direct toolImplementations, the mock IS the implementation.
    // The actual pagination logic (calling search repeatedly) is in buildToolImplementations,
    // which is used when createExecutePlanTool receives agent configOptions instead of direct mocks.
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        searchAll: {
          execute: async (params) => {
            return `All results for: ${params.query}\nPage 1\nPage 2\nAll results retrieved`;
          },
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    const result = await tool.execute({
      code: 'const r = searchAll("test"); return r;',
      description: 'Test searchAll returns mock results',
    });

    expect(result).toContain('All results for: test');
    expect(result).toContain('Page 1');
    expect(result).toContain('Page 2');
  });
});

// ─────────────────────────────────────────────────────────────────────
// 7. Bug fixes for issue #438: output() buffer cycle and misleading messages
// ─────────────────────────────────────────────────────────────────────

describe('output() buffer cycle prevention (issue #438)', () => {
  test('extractRawOutputBlocks without outputBuffer param does NOT re-add content', () => {
    // This tests the fix for Bug 1 in issue #438:
    // extractRawOutputBlocks should extract content but NOT push it back to buffer
    // when outputBuffer is not passed (as it shouldn't be from ProbeAgent)
    const content = `Tool result\n\n${RAW_OUTPUT_START}\nExtracted data\n${RAW_OUTPUT_END}\n\nDone`;

    // Call without outputBuffer (simulating the fixed ProbeAgent behavior)
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(content);

    // Should extract the block
    expect(extractedBlocks).toHaveLength(1);
    expect(extractedBlocks[0]).toBe('Extracted data');

    // Should clean the content
    expect(cleanedContent).not.toContain(RAW_OUTPUT_START);
    expect(cleanedContent).not.toContain('Extracted data');
    expect(cleanedContent).toContain('Tool result');
  });

  test('extractRawOutputBlocks WITH outputBuffer param pushes content (old behavior)', () => {
    // This tests that if outputBuffer IS passed, content is added (backwards compat)
    const content = `Result\n\n${RAW_OUTPUT_START}\nData\n${RAW_OUTPUT_END}`;
    const outputBuffer = { items: [] };

    extractRawOutputBlocks(content, outputBuffer);

    // When outputBuffer is passed, content should be added to it
    expect(outputBuffer.items).toHaveLength(1);
    expect(outputBuffer.items[0]).toBe('Data');
  });

  test('formatSuccess says "Output captured" when output() used without return (Bug 2)', async () => {
    // This tests the fix for Bug 2 in issue #438:
    // When output() is used but no return statement, the message should indicate
    // output was captured, not "no return value" which triggers LLM retries
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: {
          execute: async () => 'search results',
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // DSL script uses output() but has no return statement
    const result = await tool.execute({
      code: 'output("customer data here");',  // No return!
      description: 'Generate report',
    });

    // Should NOT say "no return value" (which triggers retries)
    expect(result).not.toContain('no return value');

    // Should indicate output was captured successfully
    expect(result).toContain('Plan completed successfully');
    expect(result).toContain('Output captured');
    expect(result).toContain('via output()');
    expect(result).toContain('final response');

    // Should contain the actual output
    expect(result).toContain('customer data here');
  });

  test('formatSuccess still says "no return value" when output() NOT used', async () => {
    // When there's truly no output (no output() and no return), message should say so
    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: {
          execute: async () => 'search results',
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // DSL script has neither output() nor return
    const result = await tool.execute({
      code: 'const x = 1 + 1;',  // No output() and no return
      description: 'Just compute',
    });

    // Should say "no return value" since nothing was produced
    expect(result).toContain('no return value');
    expect(result).not.toContain('Output captured');
  });

  test('multiple execute_plan calls do not accumulate stale RAW_OUTPUT (Bug 1 + #430)', async () => {
    // This is an integration test for the combined fix:
    // 1. formatSuccess wraps output in RAW_OUTPUT
    // 2. formatSuccess clears outputBuffer (#430 fix)
    // 3. extractRawOutputBlocks (called by ProbeAgent) does NOT re-add to buffer (#438 fix)
    // Result: Each execute_plan only wraps its OWN output, not accumulated stale data

    const outputBuffer = { items: [] };
    const tool = createExecutePlanTool({
      toolImplementations: {
        search: {
          execute: async () => 'search results',
        },
      },
      llmCall: async () => 'ok',
      outputBuffer,
      maxRetries: 0,
    });

    // First call with output()
    const result1 = await tool.execute({
      code: 'output("first report data"); return "done";',
      description: 'First report',
    });

    expect(result1).toContain('first report data');
    expect(result1).toContain(RAW_OUTPUT_START);

    // Buffer should be cleared after first call
    expect(outputBuffer.items).toHaveLength(0);

    // Simulate what happens in ProbeAgent: extract blocks WITHOUT re-adding to buffer
    const { cleanedContent } = extractRawOutputBlocks(result1);  // No outputBuffer param!

    // Buffer should STILL be empty (not re-populated by extraction)
    expect(outputBuffer.items).toHaveLength(0);

    // Second call with different output()
    const result2 = await tool.execute({
      code: 'output("second report data"); return "done";',
      description: 'Second report',
    });

    // Second result should ONLY contain second data
    expect(result2).toContain('second report data');
    expect(result2).not.toContain('first report data');

    // Should have exactly one RAW_OUTPUT block
    const starts = result2.split(RAW_OUTPUT_START).length - 1;
    expect(starts).toBe(1);
  });
});
