import { createDSLRuntime } from '../../src/agent/dsl/runtime.js';

// Mock tool implementations
function createMockTools() {
  return {
    search: {
      execute: async (params) => `search results for: ${params.query}`,
    },
    extract: {
      execute: async (params) => `extracted: ${params.targets}`,
    },
    listFiles: {
      execute: async (params) => ['file1.js', 'file2.js', 'file3.js'],
    },
  };
}

function createMockLLM() {
  return async (instruction, data, options = {}) => {
    return `LLM processed: ${instruction} with ${typeof data === 'string' ? data.substring(0, 50) : JSON.stringify(data).substring(0, 50)}`;
  };
}

describe('DSL Runtime', () => {
  let runtime;

  beforeEach(() => {
    runtime = createDSLRuntime({
      toolImplementations: createMockTools(),
      llmCall: createMockLLM(),
      mapConcurrency: 2,
    });
  });

  describe('basic execution', () => {
    test('executes simple return', async () => {
      const result = await runtime.execute('return 42;');
      expect(result.status).toBe('success');
      expect(result.result).toBe(42);
    });

    test('executes variable declarations', async () => {
      const result = await runtime.execute('const x = 1; const y = 2; return x + y;');
      expect(result.status).toBe('success');
      expect(result.result).toBe(3);
    });

    test('executes string operations', async () => {
      const result = await runtime.execute('const s = "hello"; return s.toUpperCase();');
      expect(result.status).toBe('success');
      expect(result.result).toBe('HELLO');
    });

    test('executes array operations', async () => {
      const result = await runtime.execute(`
        const arr = [3, 1, 2];
        return arr.sort();
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual([1, 2, 3]);
    });
  });

  describe('tool calls', () => {
    test('calls search tool', async () => {
      const result = await runtime.execute('const r = search("test query"); return r;');
      expect(result.status).toBe('success');
      expect(result.result).toContain('search results for: test query');
    });

    test('calls listFiles tool', async () => {
      const result = await runtime.execute('const files = listFiles("*.js"); return files;');
      expect(result.status).toBe('success');
      expect(result.result).toEqual(['file1.js', 'file2.js', 'file3.js']);
    });

    test('calls LLM', async () => {
      const result = await runtime.execute('const r = LLM("summarize", "some data"); return r;');
      expect(result.status).toBe('success');
      expect(result.result).toContain('LLM processed: summarize');
    });

    test('LLM with schema returns parsed JSON object', async () => {
      const schemaRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: async (instruction, data, options = {}) => {
          // When schema is provided, simulate the structured JSON response
          if (options.schema) {
            return '{"customers": [{"name": "Acme Corp", "api_count": "10", "use_case": "Authentication"}]}';
          }
          return `LLM processed: ${instruction}`;
        },
      });

      const result = await schemaRuntime.execute(`
        const insights = LLM(
          "Extract customer insights",
          "Some data about customers",
          { schema: '{"customers": [{"name": "string", "api_count": "string", "use_case": "string"}]}' }
        );
        return insights;
      `);
      expect(result.status).toBe('success');
      // Should be a parsed object, not a string
      expect(typeof result.result).toBe('object');
      expect(result.result.customers).toBeDefined();
      expect(result.result.customers[0].name).toBe('Acme Corp');
    });

    test('LLM with schema handles invalid JSON gracefully', async () => {
      const schemaRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: async (instruction, data, options = {}) => {
          // Simulate a malformed response
          if (options.schema) {
            return 'Not valid JSON at all';
          }
          return `LLM processed: ${instruction}`;
        },
      });

      const result = await schemaRuntime.execute(`
        const insights = LLM(
          "Extract insights",
          "Some data",
          { schema: '{"items": []}' }
        );
        return typeof insights;
      `);
      expect(result.status).toBe('success');
      // Should fall back to string if JSON parsing fails
      expect(result.result).toBe('string');
    });

    test('chains tool calls', async () => {
      const result = await runtime.execute(`
        const searchResult = search("functions");
        const summary = LLM("summarize these results", searchResult);
        return summary;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toContain('LLM processed: summarize');
    });

    test('calls bash tool when available', async () => {
      const bashRuntime = createDSLRuntime({
        toolImplementations: {
          ...createMockTools(),
          bash: {
            execute: async (params) => `command output: ${params.command}`,
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await bashRuntime.execute('const r = bash("ls -la"); return r;');
      expect(result.status).toBe('success');
      expect(result.result).toContain('command output: ls -la');
    });

    test('bash tool is not available when not provided', async () => {
      // The default runtime doesn't include bash
      const result = await runtime.execute('const r = bash("ls -la"); return r;');
      expect(result.status).toBe('error');
      expect(result.error).toContain('bash is not defined');
    });

    test('search with maxTokens parameter', async () => {
      let capturedParams = null;
      const searchRuntime = createDSLRuntime({
        toolImplementations: {
          ...createMockTools(),
          search: {
            execute: async (params) => {
              capturedParams = params;
              return `search results for: ${params.query}`;
            },
          },
        },
        llmCall: createMockLLM(),
      });

      // Test with explicit maxTokens using object syntax
      const result = await searchRuntime.execute('const r = search({query: "test", path: ".", maxTokens: 50000}); return r;');
      expect(result.status).toBe('success');
      expect(capturedParams.maxTokens).toBe(50000);
    });

    test('search with maxTokens null (unlimited)', async () => {
      let capturedParams = null;
      const searchRuntime = createDSLRuntime({
        toolImplementations: {
          ...createMockTools(),
          search: {
            execute: async (params) => {
              capturedParams = params;
              return `search results for: ${params.query}`;
            },
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await searchRuntime.execute('const r = search({query: "test", path: ".", maxTokens: null}); return r;');
      expect(result.status).toBe('success');
      expect(capturedParams.maxTokens).toBe(null);
    });

    test('searchAll is available in DSL', async () => {
      const searchAllRuntime = createDSLRuntime({
        toolImplementations: {
          ...createMockTools(),
          searchAll: {
            execute: async (params) => `all results for: ${params.query}`,
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await searchAllRuntime.execute('const r = searchAll("bulk query"); return r;');
      expect(result.status).toBe('success');
      expect(result.result).toContain('all results for: bulk query');
    });

    test('searchAll accepts options', async () => {
      let capturedParams = null;
      const searchAllRuntime = createDSLRuntime({
        toolImplementations: {
          ...createMockTools(),
          searchAll: {
            execute: async (params) => {
              capturedParams = params;
              return `all results for: ${params.query}`;
            },
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await searchAllRuntime.execute('const r = searchAll({query: "test", maxPages: 10}); return r;');
      expect(result.status).toBe('success');
      expect(capturedParams.maxPages).toBe(10);
    });
  });

  describe('map() with concurrency', () => {
    test('processes items with map()', async () => {
      const result = await runtime.execute(`
        const items = [1, 2, 3];
        const results = map(items, (item) => LLM("process", item));
        return results;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toHaveLength(3);
      expect(result.result[0]).toContain('LLM processed');
    });

    test('respects concurrency limit', async () => {
      let concurrent = 0;
      let maxConcurrent = 0;

      const slowRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: async (instruction, data) => {
          concurrent++;
          maxConcurrent = Math.max(maxConcurrent, concurrent);
          await new Promise(r => setTimeout(r, 50));
          concurrent--;
          return `done: ${data}`;
        },
        mapConcurrency: 2,
      });

      const result = await slowRuntime.execute(`
        const items = [1, 2, 3, 4, 5];
        const results = map(items, (item) => LLM("process", item));
        return results;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toHaveLength(5);
      expect(maxConcurrent).toBeLessThanOrEqual(2);
    }, 10000);
  });

  describe('utility functions', () => {
    test('chunk() splits data', async () => {
      const result = await runtime.execute(`
        const data = "a".repeat(100000);
        const chunks = chunk(data, 5000);
        return chunks.length;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBeGreaterThan(1);
    });

    test('chunkByKey() groups by key and streams efficiently', async () => {
      // Create test data with File: headers
      const result = await runtime.execute(`
        const data = [
          "File: Customers/Acme/note1.txt",
          "Acme feedback 1",
          "",
          "File: Customers/Acme/note2.txt",
          "Acme feedback 2",
          "",
          "File: Customers/Beta/note1.txt",
          "Beta feedback 1",
          "",
          "File: Customers/Gamma/note1.txt",
          "Gamma feedback 1"
        ].join("\\n");

        const chunks = chunkByKey(data, (file) => {
          const match = file.match(/Customers\\/([^\\/]+)/);
          return match ? match[1] : 'other';
        }, 100); // Small token limit for testing

        return {
          numChunks: chunks.length,
          chunk0HasAcme: chunks[0].indexOf("Acme") >= 0,
          chunk0HasBothAcmeNotes: chunks[0].indexOf("note1.txt") >= 0 && chunks[0].indexOf("note2.txt") >= 0
        };
      `);
      expect(result.status).toBe('success');
      // Both Acme notes should be in the same chunk (same key never splits)
      expect(result.result.chunk0HasAcme).toBe(true);
      expect(result.result.chunk0HasBothAcmeNotes).toBe(true);
    });

    test('chunkByKey() keeps same-key blocks together even when exceeding token limit', async () => {
      const result = await runtime.execute(`
        // Create data where a single customer has blocks that together exceed the limit
        const data = [
          "File: Customers/BigCustomer/note1.txt",
          "${"x".repeat(100)}",
          "",
          "File: Customers/BigCustomer/note2.txt",
          "${"y".repeat(100)}",
          "",
          "File: Customers/BigCustomer/note3.txt",
          "${"z".repeat(100)}"
        ].join("\\n");

        // Token limit that would be exceeded by all BigCustomer notes
        const chunks = chunkByKey(data, (file) => {
          const match = file.match(/Customers\\/([^\\/]+)/);
          return match ? match[1] : 'other';
        }, 25); // Very small limit - 100 chars

        // All BigCustomer notes should still be in one chunk (never split same key)
        return {
          numChunks: chunks.length,
          allInOneChunk: chunks[0].indexOf("note1.txt") >= 0 &&
                          chunks[0].indexOf("note2.txt") >= 0 &&
                          chunks[0].indexOf("note3.txt") >= 0
        };
      `);
      expect(result.status).toBe('success');
      expect(result.result.numChunks).toBe(1);
      expect(result.result.allInOneChunk).toBe(true);
    });

    test('chunkByKey() new key triggers flush when overflow', async () => {
      const result = await runtime.execute(`
        const data = [
          "File: Customers/Alpha/note1.txt",
          "${"a".repeat(50)}",
          "",
          "File: Customers/Alpha/note2.txt",
          "${"b".repeat(50)}",
          "",
          "File: Customers/Beta/note1.txt",
          "${"c".repeat(50)}"
        ].join("\\n");

        // Limit that fits Alpha but not Alpha + Beta
        const chunks = chunkByKey(data, (file) => {
          const match = file.match(/Customers\\/([^\\/]+)/);
          return match ? match[1] : 'other';
        }, 50); // 200 chars limit

        return {
          numChunks: chunks.length,
          chunk0HasAlpha: chunks[0].indexOf("Alpha") >= 0,
          chunk0HasBeta: chunks[0].indexOf("Beta") >= 0,
          chunk1HasBeta: chunks.length > 1 && chunks[1].indexOf("Beta") >= 0
        };
      `);
      expect(result.status).toBe('success');
      // Alpha should be in first chunk, Beta should trigger new chunk
      expect(result.result.numChunks).toBe(2);
      expect(result.result.chunk0HasAlpha).toBe(true);
      expect(result.result.chunk0HasBeta).toBe(false);
      expect(result.result.chunk1HasBeta).toBe(true);
    });

    test('chunkByKey() falls back to chunk() when no File: headers found', async () => {
      const result = await runtime.execute(`
        const data = "This is plain text without any File: headers. ".repeat(100);
        const chunks = chunkByKey(data, (file) => file, 50);
        return {
          numChunks: chunks.length,
          isArray: Array.isArray(chunks)
        };
      `);
      expect(result.status).toBe('success');
      expect(result.result.isArray).toBe(true);
      expect(result.result.numChunks).toBeGreaterThan(0);
    });

    test('chunkByKey() handles interleaved keys correctly', async () => {
      // Test that interleaved results (A1, A2, B1, B2) keep same keys together
      // The key insight: once a key is in a chunk, ALL its blocks stay there
      // So we test that A1 and A2 are together, B1 and B2 are together
      const result = await runtime.execute(`
        // Data arrives with A blocks, then B blocks (simulating grouped search results)
        const data = [
          "File: Customers/A/note1.txt",
          "${"a".repeat(60)}",
          "",
          "File: Customers/A/note2.txt",
          "${"b".repeat(60)}",
          "",
          "File: Customers/B/note1.txt",
          "${"c".repeat(60)}",
          "",
          "File: Customers/B/note2.txt",
          "${"d".repeat(60)}"
        ].join("\\n");

        // Token limit: 50 tokens = 200 chars
        // Each block ~90 chars (30 header + 60 content)
        // A1+A2 = ~180 chars (fits in 200)
        // Adding B1 would be ~270 chars (overflow) -> flush, start new chunk
        const chunks = chunkByKey(data, (file) => {
          const match = file.match(/Customers\\/([^\\/]+)/);
          return match ? match[1] : 'other';
        }, 50);

        // Verify A's are together in chunk 0
        const chunk0 = chunks[0];
        const aCount = (chunk0.match(/Customers\\/A/g) || []).length;

        // Verify B's are together in chunk 1 (if exists)
        const chunk1 = chunks[1] || '';
        const bCountInChunk1 = (chunk1.match(/Customers\\/B/g) || []).length;

        return {
          numChunks: chunks.length,
          aBlocksInChunk0: aCount,
          bBlocksInChunk1: bCountInChunk1,
          chunk0HasOnlyA: chunk0.indexOf("Customers/B") === -1,
          chunk1HasOnlyB: chunk1.indexOf("Customers/A") === -1
        };
      `);
      expect(result.status).toBe('success');
      // Should have 2 chunks - A's in first, B's in second
      expect(result.result.numChunks).toBe(2);
      // Both A blocks should be in chunk 0
      expect(result.result.aBlocksInChunk0).toBe(2);
      // Both B blocks should be in chunk 1
      expect(result.result.bBlocksInChunk1).toBe(2);
      // Chunks should not mix keys
      expect(result.result.chunk0HasOnlyA).toBe(true);
      expect(result.result.chunk1HasOnlyB).toBe(true);
    });

    test('chunkByKey() works with complex regex patterns including non-capturing groups', async () => {
      // This test verifies that complex regex patterns like (?:...) are not corrupted
      // Bug report: /^(?:Customers|Prospects)\/([^/]+)/ was getting corrupted to /0/r]+)/
      const result = await runtime.execute(`
        const data = [
          "File: Customers/Acme/note1.txt",
          "Customer note",
          "",
          "File: Prospects/Beta/note1.txt",
          "Prospect note",
          "",
          "File: Customers/Gamma/note1.txt",
          "Another customer"
        ].join("\\n");

        // Use complex regex with non-capturing group and alternation
        const chunks = chunkByKey(data, (file) => {
          const match = file.match(/^(?:Customers|Prospects)\\/([^\\/]+)/);
          return match ? match[1] : 'other';
        }, 100);

        // Verify regex worked correctly by checking extracted keys
        return {
          numChunks: chunks.length,
          chunk0: chunks[0],
          hasAcme: chunks[0].indexOf("Acme") >= 0,
          hasBeta: chunks.some(c => c.indexOf("Beta") >= 0),
          hasGamma: chunks.some(c => c.indexOf("Gamma") >= 0)
        };
      `);
      expect(result.status).toBe('success');
      // If regex was corrupted, the match would fail and all would go to 'other' key
      expect(result.result.hasAcme).toBe(true);
      expect(result.result.hasBeta).toBe(true);
      expect(result.result.hasGamma).toBe(true);
    });

    test('extractPaths() extracts unique file paths from search results', async () => {
      const result = await runtime.execute(`
        const searchResults = [
          "File: src/auth/login.js",
          "function login() {}",
          "",
          "File: src/auth/logout.js",
          "function logout() {}",
          "",
          "File: src/auth/login.js",
          "another match in same file"
        ].join("\\n");

        return extractPaths(searchResults);
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual(['src/auth/login.js', 'src/auth/logout.js']);
    });

    test('extractPaths() returns empty array when no File: headers', async () => {
      const result = await runtime.execute(`
        return extractPaths("just some text without file headers");
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual([]);
    });

    test('extractPaths() works with chunkByKey for full content workflow', async () => {
      const result = await runtime.execute(`
        const searchResults = [
          "File: Customers/Acme/note1.txt",
          "${"x".repeat(50)}",
          "",
          "File: Customers/Acme/note2.txt",
          "${"y".repeat(50)}",
          "",
          "File: Customers/Beta/note1.txt",
          "${"z".repeat(50)}"
        ].join("\\n");

        // Group by customer with small token limit to force separate chunks
        const chunks = chunkByKey(searchResults, file => {
          const match = file.match(/Customers\\/([^\\/]+)/);
          return match ? match[1] : 'other';
        }, 50); // Small limit forces Acme and Beta into separate chunks

        // Extract paths from first chunk (Acme)
        const acmePaths = extractPaths(chunks[0]);

        return {
          numChunks: chunks.length,
          acmePaths: acmePaths
        };
      `);
      expect(result.status).toBe('success');
      expect(result.result.numChunks).toBe(2);
      expect(result.result.acmePaths).toEqual(['Customers/Acme/note1.txt', 'Customers/Acme/note2.txt']);
    });

    test('range() generates array', async () => {
      const result = await runtime.execute('return range(0, 5);');
      expect(result.status).toBe('success');
      expect(result.result).toEqual([0, 1, 2, 3, 4]);
    });

    test('flatten() flattens arrays', async () => {
      const result = await runtime.execute('return flatten([[1,2],[3,4]]);');
      expect(result.status).toBe('success');
      expect(result.result).toEqual([1, 2, 3, 4]);
    });

    test('unique() deduplicates', async () => {
      const result = await runtime.execute('return unique([1,2,2,3,3,3]);');
      expect(result.status).toBe('success');
      expect(result.result).toEqual([1, 2, 3]);
    });

    test('batch() splits array into sub-arrays', async () => {
      const result = await runtime.execute('return batch([1,2,3,4,5,6,7], 3);');
      expect(result.status).toBe('success');
      expect(result.result).toEqual([[1,2,3],[4,5,6],[7]]);
    });

    test('groupBy() groups array', async () => {
      const result = await runtime.execute(`
        const items = [{type:"a", v:1}, {type:"b", v:2}, {type:"a", v:3}];
        return groupBy(items, "type");
      `);
      expect(result.status).toBe('success');
      expect(result.result.a).toHaveLength(2);
      expect(result.result.b).toHaveLength(1);
    });

    test('parseJSON() strips markdown fences from LLM output', async () => {
      const result = await runtime.execute(`
        var raw = '  \\\`\\\`\\\`json\\n[{"name":"a"},{"name":"b"}]\\n\\\`\\\`\\\`  ';
        return parseJSON(raw);
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual([{ name: 'a' }, { name: 'b' }]);
    });

    test('parseJSON() handles clean JSON without fences', async () => {
      const result = await runtime.execute(`
        return parseJSON('{"key": "value"}');
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual({ key: 'value' });
    });

    test('parseJSON() extracts JSON from surrounding text', async () => {
      const result = await runtime.execute(`
        return parseJSON('Here is the result: [{"a":1}] end');
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual([{ a: 1 }]);
    });

    test('log() collects messages', async () => {
      const result = await runtime.execute(`
        log("step 1");
        log("step 2");
        return "done";
      `);
      expect(result.status).toBe('success');
      expect(result.logs).toContain('step 1');
      expect(result.logs).toContain('step 2');
    });
  });

  describe('validation errors', () => {
    test('rejects async keyword', async () => {
      const result = await runtime.execute('const fn = async () => 1;');
      expect(result.status).toBe('error');
      expect(result.error).toContain('Validation failed');
    });

    test('rejects eval', async () => {
      const result = await runtime.execute('eval("1+1");');
      expect(result.status).toBe('error');
      expect(result.error).toContain('Validation failed');
    });

    test('rejects require', async () => {
      const result = await runtime.execute('const fs = require("fs");');
      expect(result.status).toBe('error');
      expect(result.error).toContain('Validation failed');
    });

    test('rejects class', async () => {
      const result = await runtime.execute('class Foo {}');
      expect(result.status).toBe('error');
      expect(result.error).toContain('Validation failed');
    });
  });

  describe('loop guards', () => {
    test('stops infinite while loops', async () => {
      const guardedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        maxLoopIterations: 10,
      });

      const result = await guardedRuntime.execute(`
        let i = 0;
        while (true) {
          i = i + 1;
        }
        return i;
      `);
      expect(result.status).toBe('error');
      expect(result.error).toContain('Loop exceeded maximum');
    });

    test('stops runaway for loops', async () => {
      const guardedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        maxLoopIterations: 5,
      });

      const result = await guardedRuntime.execute(`
        let sum = 0;
        for (let i = 0; i < 100; i = i + 1) {
          sum = sum + i;
        }
        return sum;
      `);
      expect(result.status).toBe('error');
      expect(result.error).toContain('Loop exceeded maximum');
    });

    test('allows loops within limit', async () => {
      const guardedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        maxLoopIterations: 100,
      });

      const result = await guardedRuntime.execute(`
        let sum = 0;
        for (let i = 0; i < 10; i = i + 1) {
          sum = sum + i;
        }
        return sum;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe(45);
    });

    test('counts iterations across multiple loops', async () => {
      const guardedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        maxLoopIterations: 15,
      });

      const result = await guardedRuntime.execute(`
        let a = 0;
        for (let i = 0; i < 10; i = i + 1) {
          a = a + 1;
        }
        let b = 0;
        for (let j = 0; j < 10; j = j + 1) {
          b = b + 1;
        }
        return a + b;
      `);
      expect(result.status).toBe('error');
      expect(result.error).toContain('Loop exceeded maximum');
    });
  });

  // NOTE: SandboxJS has a known issue with async error propagation.
  // Errors thrown inside async globals escape the promise chain as unhandled rejections
  // instead of being caught by our try/catch around exec().run().
  // This will be addressed in a future iteration — options include:
  // 1. Wrapping tool globals to return error objects instead of throwing
  // 2. Using process.on('unhandledRejection') to capture escaping errors
  // 3. Using SandboxJS sync mode (compile instead of compileAsync) with a different approach
  // For now, tool implementations should not throw — they should return error values.

  describe('OTEL tracing', () => {
    function createMockTracer() {
      const spans = [];
      const events = [];
      return {
        spans,
        events,
        createToolSpan: (name, attrs = {}) => {
          const span = {
            name,
            attributes: { ...attrs },
            events: [],
            status: null,
            ended: false,
            setAttributes: (a) => Object.assign(span.attributes, a),
            setStatus: (s) => { span.status = s; },
            addEvent: (n, a = {}) => { span.events.push({ name: n, ...a }); },
            end: () => { span.ended = true; },
          };
          spans.push(span);
          return span;
        },
        addEvent: (name, attrs = {}) => {
          events.push({ name, ...attrs });
        },
        recordToolResult: (toolName, result, success, durationMs, metadata = {}) => {
          events.push({ name: 'tool.result', toolName, success, durationMs, ...metadata });
        },
      };
    }

    test('traces individual tool calls (search, LLM)', async () => {
      const mockTracer = createMockTracer();
      const tracedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        tracer: mockTracer,
      });

      const result = await tracedRuntime.execute(`
        const r = search("test");
        const summary = LLM("summarize", r);
        return summary;
      `);

      expect(result.status).toBe('success');

      // Should have spans for dsl.search and dsl.LLM
      const toolSpanNames = mockTracer.spans.map(s => s.name);
      expect(toolSpanNames).toContain('dsl.search');
      expect(toolSpanNames).toContain('dsl.LLM');

      // All spans should be ended
      for (const span of mockTracer.spans) {
        expect(span.ended).toBe(true);
        expect(span.status).toBe('OK');
      }

      // Should have tool.result events
      const resultEvents = mockTracer.events.filter(e => e.name === 'tool.result');
      expect(resultEvents.length).toBeGreaterThanOrEqual(2);
    });

    test('traces map() calls', async () => {
      const mockTracer = createMockTracer();
      const tracedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        tracer: mockTracer,
      });

      const result = await tracedRuntime.execute(`
        const items = [1, 2, 3];
        const results = map(items, (item) => LLM("process", item));
        return results;
      `);

      expect(result.status).toBe('success');

      // Should have a dsl.map span + 3 dsl.LLM spans
      const mapSpans = mockTracer.spans.filter(s => s.name === 'dsl.map');
      const llmSpans = mockTracer.spans.filter(s => s.name === 'dsl.LLM');
      expect(mapSpans.length).toBe(1);
      expect(llmSpans.length).toBe(3);
    });

    test('records runtime phase events', async () => {
      const mockTracer = createMockTracer();
      const tracedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        tracer: mockTracer,
      });

      await tracedRuntime.execute('return 42;');

      // Should have validate, transform, execute phase events
      const eventNames = mockTracer.events.map(e => e.name);
      expect(eventNames).toContain('dsl.phase.validate_start');
      expect(eventNames).toContain('dsl.phase.validate_complete');
      expect(eventNames).toContain('dsl.phase.transform_start');
      expect(eventNames).toContain('dsl.phase.transform_complete');
      expect(eventNames).toContain('dsl.phase.execute_start');
      expect(eventNames).toContain('dsl.phase.execute_complete');
    });

    test('records failure events on error', async () => {
      const mockTracer = createMockTracer();
      const tracedRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        tracer: mockTracer,
      });

      await tracedRuntime.execute('eval("bad")');

      const eventNames = mockTracer.events.map(e => e.name);
      expect(eventNames).toContain('dsl.phase.validate_failed');
    });
  });

  describe('session store', () => {
    test('storeSet and storeGet within a single execution', async () => {
      const result = await runtime.execute(`
        storeSet("key1", "value1");
        return storeGet("key1");
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe('value1');
    });

    test('store persists across multiple execute() calls', async () => {
      const r1 = await runtime.execute('storeSet("counter", 42); return "stored";');
      expect(r1.status).toBe('success');

      const r2 = await runtime.execute('return storeGet("counter");');
      expect(r2.status).toBe('success');
      expect(r2.result).toBe(42);
    });

    test('storeAppend creates array and accumulates items', async () => {
      const storeRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });

      await storeRuntime.execute('storeAppend("items", "a");');
      await storeRuntime.execute('storeAppend("items", "b");');
      await storeRuntime.execute('storeAppend("items", "c");');

      const result = await storeRuntime.execute('return storeGet("items");');
      expect(result.status).toBe('success');
      expect(result.result).toEqual(['a', 'b', 'c']);
    });

    test('storeKeys returns all stored keys', async () => {
      const storeRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });

      await storeRuntime.execute('storeSet("x", 1); storeSet("y", 2);');
      const result = await storeRuntime.execute('return storeKeys();');
      expect(result.status).toBe('success');
      expect(result.result.sort()).toEqual(['x', 'y']);
    });

    test('storeGetAll returns copy of entire store', async () => {
      const storeRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });

      await storeRuntime.execute('storeSet("a", 1); storeSet("b", 2);');
      const result = await storeRuntime.execute('return storeGetAll();');
      expect(result.status).toBe('success');
      expect(result.result).toEqual({ a: 1, b: 2 });
    });

    test('storeGet returns undefined for missing keys', async () => {
      const result = await runtime.execute('return storeGet("nonexistent");');
      expect(result.status).toBe('success');
      expect(result.result).toBeUndefined();
    });

    test('storeSet overwrites existing values', async () => {
      const storeRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });

      await storeRuntime.execute('storeSet("key", "old");');
      await storeRuntime.execute('storeSet("key", "new");');
      const result = await storeRuntime.execute('return storeGet("key");');
      expect(result.status).toBe('success');
      expect(result.result).toBe('new');
    });

    test('storeAppend works with objects', async () => {
      const storeRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });

      await storeRuntime.execute('storeAppend("items", {name: "a", value: 1});');
      await storeRuntime.execute('storeAppend("items", {name: "b", value: 2});');
      const result = await storeRuntime.execute('return storeGet("items");');
      expect(result.status).toBe('success');
      expect(result.result).toEqual([
        { name: 'a', value: 1 },
        { name: 'b', value: 2 },
      ]);
    });

    test('different runtime instances have separate stores', async () => {
      const runtime1 = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });
      const runtime2 = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
      });

      await runtime1.execute('storeSet("session", "runtime1");');
      await runtime2.execute('storeSet("session", "runtime2");');

      const r1 = await runtime1.execute('return storeGet("session");');
      const r2 = await runtime2.execute('return storeGet("session");');

      expect(r1.result).toBe('runtime1');
      expect(r2.result).toBe('runtime2');
    });

    test('data pipeline pattern with store', async () => {
      const pipelineRuntime = createDSLRuntime({
        toolImplementations: {
          search: {
            execute: async () => 'GET /users\nPOST /users\nDELETE /users/:id',
          },
        },
        llmCall: async (instruction, data) => {
          if (instruction.includes('Extract')) {
            return JSON.stringify([
              { method: 'GET', path: '/users' },
              { method: 'POST', path: '/users' },
              { method: 'DELETE', path: '/users/:id' },
            ]);
          }
          return 'Summary: 3 endpoints found';
        },
      });

      const result = await pipelineRuntime.execute(`
        const results = search("endpoints");
        const chunks = chunk(results);
        for (const c of chunks) {
          var parsed = JSON.parse(String(LLM("Extract endpoints as JSON", c)));
          for (const item of parsed) { storeAppend("endpoints", item); }
        }
        var all = storeGet("endpoints");
        log("Total: " + all.length);
        var byMethod = groupBy(all, "method");
        return { total: all.length, methods: Object.keys(byMethod) };
      `);
      expect(result.status).toBe('success');
      expect(result.result.total).toBe(3);
      expect(result.result.methods.sort()).toEqual(['DELETE', 'GET', 'POST']);
    });
  });

  describe('error-safe tool returns', () => {
    test('tool error returns ERROR: string instead of throwing', async () => {
      const errorRuntime = createDSLRuntime({
        toolImplementations: {
          search: {
            execute: async () => { throw new Error('search failed'); },
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await errorRuntime.execute(`
        const r = search("test");
        return r;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe('ERROR: search failed');
    });

    test('parseJSON returns null on invalid input', async () => {
      const result = await runtime.execute(`
        const r = parseJSON("not valid json");
        return r;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBeNull();
    });

    test('error-resilient loop via string check', async () => {
      const errorRuntime = createDSLRuntime({
        toolImplementations: {
          extract: {
            execute: async (params) => {
              if (params.targets === 'bad.js') throw new Error('file not found');
              return 'content of ' + params.targets;
            },
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await errorRuntime.execute(`
        const files = ["good.js", "bad.js", "other.js"];
        const results = [];
        for (const f of files) {
          const content = extract(f);
          if (typeof content === "string" && content.indexOf("ERROR:") === 0) {
            results.push("err: " + f);
          } else {
            results.push("ok: " + f);
          }
        }
        return results;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toEqual([
        'ok: good.js',
        'err: bad.js',
        'ok: other.js',
      ]);
    });

    test('LLM error returns ERROR: string', async () => {
      const errorRuntime = createDSLRuntime({
        toolImplementations: {},
        llmCall: async () => { throw new Error('API rate limited'); },
      });

      const result = await errorRuntime.execute(`
        const r = LLM("test", "data");
        return r;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe('ERROR: API rate limited');
    });

    test('errors are logged', async () => {
      const errorRuntime = createDSLRuntime({
        toolImplementations: {
          search: {
            execute: async () => { throw new Error('timeout'); },
          },
        },
        llmCall: createMockLLM(),
      });

      const result = await errorRuntime.execute(`
        search("test");
        return "done";
      `);
      expect(result.status).toBe('success');
      expect(result.logs.some(l => l.includes('[search]') && l.includes('ERROR: timeout'))).toBe(true);
    });
  });

  describe('end-to-end scenario', () => {
    test('analyze_all replacement pattern', async () => {
      const analyzeRuntime = createDSLRuntime({
        toolImplementations: {
          search: {
            execute: async (params) => {
              return `File1:\nfunction getUser() {}\n\nFile2:\nfunction deleteUser() {}\n\nFile3:\nfunction updateUser() {}`;
            },
          },
        },
        llmCall: async (instruction, data) => {
          if (instruction.includes('Extract')) {
            return ['getUser', 'deleteUser', 'updateUser'];
          }
          if (instruction.includes('Organize')) {
            return { crud: { read: ['getUser'], delete: ['deleteUser'], update: ['updateUser'] } };
          }
          return data;
        },
        mapConcurrency: 3,
      });

      const result = await analyzeRuntime.execute(`
        const results = search("user functions", "./src");
        const chunks = chunk(results);
        const extracted = map(chunks, (c) => LLM("Extract function names", c));
        return LLM("Organize by CRUD operation", flatten(extracted));
      `);

      expect(result.status).toBe('success');
      expect(result.result).toHaveProperty('crud');
      expect(result.result.crud.read).toContain('getUser');
    }, 10000);
  });

  describe('output buffer', () => {
    test('output() writes to buffer', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      const result = await bufRuntime.execute(`
        output("hello world");
        return "done";
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe('done');
      expect(outputBuffer.items).toEqual(['hello world']);
    });

    test('output() can be called multiple times', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      const result = await bufRuntime.execute(`
        output("line 1");
        output("line 2");
        output("line 3");
        return "done";
      `);
      expect(result.status).toBe('success');
      expect(outputBuffer.items).toEqual(['line 1', 'line 2', 'line 3']);
    });

    test('output() stringifies non-string content', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      const result = await bufRuntime.execute(`
        output({name: "test", value: 42});
        return "done";
      `);
      expect(result.status).toBe('success');
      expect(outputBuffer.items).toHaveLength(1);
      const parsed = JSON.parse(outputBuffer.items[0]);
      expect(parsed).toEqual({ name: 'test', value: 42 });
    });

    test('output() ignores null and undefined', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      const result = await bufRuntime.execute(`
        output(null);
        output(undefined);
        output("valid");
        return "done";
      `);
      expect(result.status).toBe('success');
      expect(outputBuffer.items).toEqual(['valid']);
    });

    test('output buffer persists across multiple execute() calls', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      await bufRuntime.execute('output("from call 1");');
      await bufRuntime.execute('output("from call 2");');

      expect(outputBuffer.items).toEqual(['from call 1', 'from call 2']);
    });

    test('output() and return are independent', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      const result = await bufRuntime.execute(`
        output("| Col1 | Col2 |\\n| a | b |");
        return "Table with 1 row generated";
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe('Table with 1 row generated');
      expect(outputBuffer.items[0]).toContain('| Col1 | Col2 |');
    });

    test('output() not available when no outputBuffer provided', async () => {
      const result = await runtime.execute(`
        if (typeof output === "undefined") {
          return "output not available";
        }
        output("test");
        return "output available";
      `);
      expect(result.status).toBe('success');
      expect(result.result).toBe('output not available');
    });

    test('output() logs buffer write notification', async () => {
      const outputBuffer = { items: [] };
      const bufRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        outputBuffer,
      });

      const result = await bufRuntime.execute(`
        output("some content");
        return "done";
      `);
      expect(result.status).toBe('success');
      const outputLog = result.logs.find(l => l.startsWith('[output]'));
      expect(outputLog).toBeDefined();
      expect(outputLog).toContain('chars written to output buffer');
    });
  });
});

// Test extractRawOutputBlocks helper function
import { extractRawOutputBlocks, RAW_OUTPUT_START, RAW_OUTPUT_END } from '../../src/tools/executePlan.js';

describe('extractRawOutputBlocks', () => {
  test('extracts single raw output block', () => {
    const content = `Some result text\n\n${RAW_OUTPUT_START}\nCSV data here\nline2,data\n${RAW_OUTPUT_END}\n\n[The above raw output (20 chars) will be passed directly to the final response. Do NOT repeat, summarize, or modify it.]`;

    const outputBuffer = { items: [] };
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(content, outputBuffer);

    expect(extractedBlocks).toHaveLength(1);
    expect(extractedBlocks[0]).toBe('CSV data here\nline2,data');
    expect(outputBuffer.items).toHaveLength(1);
    expect(outputBuffer.items[0]).toBe('CSV data here\nline2,data');
    expect(cleanedContent).not.toContain(RAW_OUTPUT_START);
    expect(cleanedContent).not.toContain(RAW_OUTPUT_END);
    expect(cleanedContent).not.toContain('Do NOT repeat');
  });

  test('extracts multiple raw output blocks', () => {
    const content = `Result 1\n\n${RAW_OUTPUT_START}\nBlock 1\n${RAW_OUTPUT_END}\n\nSome text\n\n${RAW_OUTPUT_START}\nBlock 2\n${RAW_OUTPUT_END}`;

    const outputBuffer = { items: [] };
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(content, outputBuffer);

    expect(extractedBlocks).toHaveLength(2);
    expect(extractedBlocks[0]).toBe('Block 1');
    expect(extractedBlocks[1]).toBe('Block 2');
    expect(outputBuffer.items).toHaveLength(2);
  });

  test('returns original content when no blocks present', () => {
    const content = 'Just regular content without any blocks';
    const outputBuffer = { items: [] };
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(content, outputBuffer);

    expect(extractedBlocks).toHaveLength(0);
    expect(cleanedContent).toBe(content);
    expect(outputBuffer.items).toHaveLength(0);
  });

  test('works without outputBuffer parameter', () => {
    const content = `Text\n\n${RAW_OUTPUT_START}\nData\n${RAW_OUTPUT_END}`;
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(content);

    expect(extractedBlocks).toHaveLength(1);
    expect(extractedBlocks[0]).toBe('Data');
    expect(cleanedContent).toBe('Text');
  });

  test('handles non-string content', () => {
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(null);
    expect(cleanedContent).toBeNull();
    expect(extractedBlocks).toHaveLength(0);

    const { cleanedContent: c2, extractedBlocks: e2 } = extractRawOutputBlocks(123);
    expect(c2).toBe(123);
    expect(e2).toHaveLength(0);
  });

  test('preserves multiline content in blocks', () => {
    const multilineData = 'customer,value,status\nAcme,100,active\nBeta,200,pending\nGamma,300,active';
    const content = `Plan executed\n\n${RAW_OUTPUT_START}\n${multilineData}\n${RAW_OUTPUT_END}`;

    const { extractedBlocks } = extractRawOutputBlocks(content);
    expect(extractedBlocks[0]).toBe(multilineData);
  });

  test('simulates nested agent passthrough - child output cascades to parent', () => {
    // Simulate child agent's execute_plan returning output with RAW_OUTPUT delimiters
    const childToolResult = `Plan: Generate customer report

Result: Report generated successfully

${RAW_OUTPUT_START}
customer,revenue,status
Acme Corp,50000,active
Beta Inc,30000,pending
Gamma LLC,75000,active
${RAW_OUTPUT_END}

[The above raw output (89 chars) will be passed directly to the final response. Do NOT repeat, summarize, or modify it.]`;

    // Parent agent has its own output buffer
    const parentOutputBuffer = { items: [] };

    // When parent processes the tool result, it extracts raw blocks
    const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(childToolResult, parentOutputBuffer);

    // Raw content should be in parent's output buffer now
    expect(parentOutputBuffer.items).toHaveLength(1);
    expect(parentOutputBuffer.items[0]).toContain('customer,revenue,status');
    expect(parentOutputBuffer.items[0]).toContain('Acme Corp,50000,active');

    // Cleaned content should not have the raw block (LLM sees summary only)
    expect(cleanedContent).toContain('Plan: Generate customer report');
    expect(cleanedContent).toContain('Result: Report generated successfully');
    expect(cleanedContent).not.toContain('Acme Corp,50000,active');
    expect(cleanedContent).not.toContain(RAW_OUTPUT_START);
  });

  test('simulates multi-level nesting - grandchild to parent passthrough', () => {
    // Level 1: Grandchild produces raw output
    const grandchildOutput = { items: [] };
    grandchildOutput.items.push('| Customer | Value |\n| Acme | 100 |');

    // Level 2: Child wraps grandchild's output and produces its own tool result
    // (simulating what formatSuccess does)
    const childToolResult = `Plan: Aggregate data

Result: Aggregated 1 customer

${RAW_OUTPUT_START}
| Customer | Value |
| Acme | 100 |
${RAW_OUTPUT_END}

[The above raw output (32 chars) will be passed directly to the final response. Do NOT repeat, summarize, or modify it.]`;

    // Level 3: Parent extracts and accumulates
    const parentOutputBuffer = { items: ['Previous output from parent'] };
    const { cleanedContent } = extractRawOutputBlocks(childToolResult, parentOutputBuffer);

    // Parent should have both its own output AND child's raw output
    expect(parentOutputBuffer.items).toHaveLength(2);
    expect(parentOutputBuffer.items[0]).toBe('Previous output from parent');
    expect(parentOutputBuffer.items[1]).toContain('| Acme | 100 |');

    // LLM only sees the summary
    expect(cleanedContent).toContain('Aggregated 1 customer');
    expect(cleanedContent).not.toContain('| Acme | 100 |');
  });
});
