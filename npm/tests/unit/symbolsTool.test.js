import { jest, describe, test, expect, beforeEach } from '@jest/globals';

// Mock all the heavy dependencies that ProbeAgent uses
jest.mock('@ai-sdk/anthropic', () => ({}));
jest.mock('@ai-sdk/openai', () => ({}));
jest.mock('@ai-sdk/google', () => ({}));
jest.mock('@ai-sdk/amazon-bedrock', () => ({}));
jest.mock('ai', () => ({
  generateText: jest.fn(),
  streamText: jest.fn(),
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('symbolsSchema', () => {
  let symbolsSchema;

  beforeEach(async () => {
    const mod = await import('../../src/tools/common.js');
    symbolsSchema = mod.symbolsSchema;
  });

  test('validates valid input', () => {
    const result = symbolsSchema.safeParse({ file: 'src/main.rs' });
    expect(result.success).toBe(true);
  });

  test('rejects missing file', () => {
    const result = symbolsSchema.safeParse({});
    expect(result.success).toBe(false);
  });

  test('rejects non-string file', () => {
    const result = symbolsSchema.safeParse({ file: 123 });
    expect(result.success).toBe(false);
  });
});

describe('symbols tool in agent', () => {
  let agent;

  beforeEach(() => {
    agent = new ProbeAgent({
      provider: 'anthropic',
      model: 'claude-sonnet-4-20250514',
      path: '.',
    });
  });

  test('symbols tool is registered in toolImplementations', () => {
    expect(agent.toolImplementations.symbols).toBeDefined();
    expect(agent.toolImplementations.symbols.execute).toBeInstanceOf(Function);
  });

  test('symbols tool has correct schema and description', () => {
    const toolInfo = agent._getToolSchemaAndDescription('symbols');
    expect(toolInfo).toBeDefined();
    expect(toolInfo.description).toContain('symbols');
    expect(toolInfo.schema).toBeDefined();
  });
});

describe('symbols exports', () => {
  test('symbolsSchema is exported from agent/tools.js', async () => {
    const mod = await import('../../src/agent/tools.js');
    expect(mod.symbolsSchema).toBeDefined();
  });

  test('symbolsTool and symbolsSchema are exported from index.js', async () => {
    const mod = await import('../../src/index.js');
    expect(mod.symbolsSchema).toBeDefined();
    expect(mod.symbolsTool).toBeDefined();
  });
});
