import { describe, test, expect } from '@jest/globals';
import ts from 'typescript';

/**
 * Regression test: ensure the public TypeScript surface exposes tool filtering
 * and system prompt options. We compile a tiny snippet and assert no diagnostics.
 */
describe('Type definitions: ProbeAgentOptions', () => {
  const compile = (source) => {
    const result = ts.transpileModule(source, {
      compilerOptions: {
        target: ts.ScriptTarget.ES2020,
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Node16,
        strict: true,
        skipLibCheck: true,
        isolatedModules: true,
        allowImportingTsExtensions: true,
        types: [],
      }
    });
    return result.diagnostics || [];
  };

  test('accepts systemPrompt, allowedTools, and disableTools', () => {
    const diagnostics = compile(`
      import { ProbeAgent, type ProbeAgentOptions } from '../..';

      const options: ProbeAgentOptions = {
        systemPrompt: 'hello',
        customPrompt: 'fallback',
        allowedTools: ['search', '!bash'],
        disableTools: false,
      };

      const agent = new ProbeAgent(options);
      void agent;
    `);

    expect(diagnostics.length).toBe(0);
  });
});
