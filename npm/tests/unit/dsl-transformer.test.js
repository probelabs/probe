import { transformDSL } from '../../src/agent/dsl/transformer.js';

const ASYNC_FUNCS = new Set(['search', 'query', 'extract', 'LLM', 'map', 'listFiles', 'searchFiles', 'bash', 'mcp_github_create_issue']);

describe('DSL Transformer', () => {
  describe('await injection', () => {
    test('injects await before async function calls', () => {
      const code = 'const r = search("query");';
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('await search("query")');
    });

    test('injects await before multiple calls', () => {
      const code = `
        const a = search("foo");
        const b = LLM("summarize", a);
        return b;
      `;
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('await search("foo")');
      expect(result).toContain('await LLM("summarize", a)');
    });

    test('does not inject await before non-async functions', () => {
      const code = 'const x = chunk(data, 20000); return x;';
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).not.toContain('await chunk');
      expect(result).toContain('chunk(data, 20000)');
    });

    test('injects await before MCP tool calls', () => {
      const code = 'const issue = mcp_github_create_issue({ title: "test" });';
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('await mcp_github_create_issue');
    });

    test('handles map() with callback containing async calls', () => {
      const code = `
        const results = map(items, (item) => LLM("process", item));
      `;
      const result = transformDSL(code, ASYNC_FUNCS);
      // map itself should be awaited
      expect(result).toContain('await map');
      // The callback should be marked async
      expect(result).toContain('async (item)');
      // LLM inside callback should be awaited
      expect(result).toContain('await LLM("process", item)');
    });
  });

  describe('async IIFE wrapping', () => {
    test('wraps code in async IIFE with return', () => {
      const code = 'return 42;';
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toMatch(/^return \(async \(\) => \{/);
      expect(result).toMatch(/\}\)\(\)$/);
    });
  });

  describe('complex programs', () => {
    test('typical analyze_all replacement', () => {
      const code = `
const results = search("API endpoints", "./src");
const chunks = chunk(results, 20000);
const extracted = map(chunks, (c) => LLM("Extract endpoints", c));
return LLM("Organize", extracted);
      `.trim();
      const result = transformDSL(code, ASYNC_FUNCS);

      // search should be awaited
      expect(result).toContain('await search("API endpoints"');
      // chunk should NOT be awaited (it's sync)
      expect(result).not.toContain('await chunk(');
      // map should be awaited
      expect(result).toContain('await map(');
      // LLM in callback should be awaited
      expect(result).toContain('await LLM("Extract endpoints"');
      // Final LLM should be awaited
      expect(result).toContain('await LLM("Organize"');
    });

    test('preserves code structure', () => {
      const code = `
const items = [1, 2, 3];
if (items.length > 0) {
  const result = search("test");
  return result;
}
return null;
      `.trim();
      const result = transformDSL(code, ASYNC_FUNCS);
      // Structure should be preserved
      expect(result).toContain('if (items.length > 0)');
      expect(result).toContain('await search("test")');
      expect(result).toContain('return null');
    });

    test('handles nested function calls', () => {
      const code = 'return LLM("process", search("query"));';
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('await LLM');
      expect(result).toContain('await search');
    });
  });

  describe('catch parameter fix', () => {
    test('injects __getLastError() call in catch body', () => {
      const code = `
        try {
          search("test");
        } catch (e) {
          log("error: " + e);
        }
      `;
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('catch (__catchParam)');
      expect(result).toContain('var e = __getLastError();');
    });

    test('handles different catch parameter names', () => {
      const code = `
        try {
          search("test");
        } catch (err) {
          log(err);
        }
      `;
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('catch (__catchParam)');
      expect(result).toContain('var err = __getLastError();');
    });

    test('does not inject for catch without parameter', () => {
      const code = `
        try {
          search("test");
        } catch {
          log("caught");
        }
      `;
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).not.toContain('__getLastError');
    });

    test('transforms throw statement to capture via __setLastError', () => {
      const code = `throw "custom error";`;
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('throw __setLastError("custom error")');
    });

    test('handles nested try/catch', () => {
      const code = `
        try {
          try {
            search("test");
          } catch (inner) {
            log(inner);
          }
        } catch (outer) {
          log(outer);
        }
      `;
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).toContain('var inner = __getLastError();');
      expect(result).toContain('var outer = __getLastError();');
      // Both catch params should be renamed to __catchParam
      expect(result.match(/__catchParam/g).length).toBe(2);
    });
  });

  describe('edge cases', () => {
    test('handles empty code', () => {
      expect(() => transformDSL('', ASYNC_FUNCS)).not.toThrow();
    });

    test('handles code with no async calls', () => {
      const code = 'const x = 1 + 2; return x;';
      const result = transformDSL(code, ASYNC_FUNCS);
      // Should still wrap in async IIFE
      expect(result).toContain('async');
      // But no await should be inserted
      expect(result).not.toContain('await');
    });

    test('handles code with only utility calls', () => {
      const code = 'const r = range(0, 10); return flatten([r, r]);';
      const result = transformDSL(code, ASYNC_FUNCS);
      expect(result).not.toContain('await range');
      expect(result).not.toContain('await flatten');
    });
  });
});
