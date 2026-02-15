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

    test('chains tool calls', async () => {
      const result = await runtime.execute(`
        const searchResult = search("functions");
        const summary = LLM("summarize these results", searchResult);
        return summary;
      `);
      expect(result.status).toBe('success');
      expect(result.result).toContain('LLM processed: summarize');
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

  describe('timeout and loop guards', () => {
    test('times out long-running execution', async () => {
      const slowRuntime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: async (instruction, data) => {
          await new Promise(r => setTimeout(r, 5000));
          return 'done';
        },
        timeoutMs: 500,
      });

      const result = await slowRuntime.execute(`
        const r = LLM("slow", "data");
        return r;
      `);
      expect(result.status).toBe('error');
      expect(result.error).toContain('timed out');
    }, 10000);

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
