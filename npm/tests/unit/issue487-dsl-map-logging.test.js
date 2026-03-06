import { generateSandboxGlobals } from '../../src/agent/dsl/environment.js';

describe('Issue #487 - DSL map concurrency visibility', () => {
  test('logs when map() hits concurrency limit even without debug mode', async () => {
    const globals = generateSandboxGlobals({
      toolImplementations: {},
      llmCall: async (_instruction, data) => {
        await new Promise(resolve => setTimeout(resolve, 25));
        return `processed:${data}`;
      },
      mapConcurrency: 1
    });

    const result = await globals.map([1, 2, 3], (item) => globals.LLM('process', item));

    expect(result).toEqual(['processed:1', 'processed:2', 'processed:3']);
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining('[map] Concurrency limit reached')
    );
  });
});
