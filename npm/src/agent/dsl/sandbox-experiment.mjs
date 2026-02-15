/**
 * Quick experiment to verify SandboxJS capabilities for our DSL runtime.
 *
 * Tests:
 * 1. compileAsync() with host async functions as globals
 * 2. Error propagation from sandbox to host
 * 3. Tick limits
 * 4. Sandbox.audit() for introspection
 * 5. map() concurrency pattern
 * 6. Nested async calls (callback inside map that calls async)
 */

import SandboxModule from '@nyariv/sandboxjs';
const Sandbox = SandboxModule.default || SandboxModule;

async function test(name, fn) {
  try {
    const result = await fn();
    console.log(`PASS: ${name}`, result !== undefined ? `→ ${JSON.stringify(result)}` : '');
  } catch (e) {
    console.log(`FAIL: ${name} → ${e.message}`);
  }
}

// Test 1: Basic async function as global
await test('Host async function as global', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      fetchData: async (query) => {
        return { results: [`result for: ${query}`] };
      }
    }
  });
  const exec = s.compileAsync(`
    const data = await fetchData("test query");
    return data.results[0];
  `);
  const result = await exec().run();
  if (result !== 'result for: test query') throw new Error(`Expected 'result for: test query', got '${result}'`);
  return result;
});

// Test 2: Multiple sequential async calls
await test('Multiple sequential async calls', async () => {
  const callLog = [];
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      step: async (n) => {
        callLog.push(n);
        return `done-${n}`;
      }
    }
  });
  const exec = s.compileAsync(`
    const a = await step(1);
    const b = await step(2);
    const c = await step(3);
    return a + "," + b + "," + c;
  `);
  const result = await exec().run();
  if (callLog.join(',') !== '1,2,3') throw new Error(`Wrong call order: ${callLog}`);
  return result;
});

// Test 3: Error propagation
await test('Error propagation from async global', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      failingTool: async () => {
        throw new Error('Tool failed!');
      }
    }
  });
  const exec = s.compileAsync(`
    const result = await failingTool();
    return result;
  `);
  try {
    await exec().run();
    throw new Error('Should have thrown');
  } catch (e) {
    if (!e.message.includes('Tool failed')) throw new Error(`Wrong error: ${e.message}`);
    return 'Error correctly propagated';
  }
});

// Test 4: Sandbox.audit()
await test('Sandbox.audit() reports accessed globals', async () => {
  const audit = Sandbox.audit(`
    const x = myFunc("test");
    const y = otherFunc(x);
    return y;
  `);
  return audit;
});

// Test 5: Code without await (what LLM would write, before our transform)
await test('Async global called WITHOUT await', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      fetchData: async (query) => {
        return { results: [`result for: ${query}`] };
      }
    }
  });
  // Without await - should return a Promise object, not resolved value
  const exec = s.compileAsync(`
    const data = fetchData("test");
    return data;
  `);
  const result = await exec().run();
  // Check if it's a Promise (unresolved) or the actual value
  const isPromise = result && typeof result.then === 'function';
  return { isPromise, type: typeof result, value: isPromise ? 'Promise (unresolved)' : result };
});

// Test 6: Custom throw for pause-like mechanism (just to verify throws work)
await test('Custom throw propagation', async () => {
  class PauseSignal {
    constructor(value) { this.value = value; this.isPause = true; }
  }
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      pause: (value) => { throw new PauseSignal(value); }
    }
  });
  const exec = s.compileAsync(`
    const x = 42;
    pause({ result: x });
    return "should not reach here";
  `);
  try {
    await exec().run();
    throw new Error('Should have thrown');
  } catch (e) {
    if (e.isPause) {
      return { paused: true, value: e.value };
    }
    return { paused: false, error: e.message };
  }
});

// Test 7: for...of loop with async
await test('for...of with async calls', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      process: async (item) => item * 2  // Note: 'process' as a name - might conflict
    }
  });
  const exec = s.compileAsync(`
    const items = [1, 2, 3, 4, 5];
    const results = [];
    for (const item of items) {
      results.push(await process(item));
    }
    return results;
  `);
  const result = await exec().run();
  const expected = [2, 4, 6, 8, 10];
  if (JSON.stringify(result) !== JSON.stringify(expected)) throw new Error(`Got ${JSON.stringify(result)}`);
  return result;
});

// Test 8: Passing scope variables
await test('Scope variables accessible in sandbox', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      transform: async (x) => x.toUpperCase()
    }
  });
  const exec = s.compileAsync(`
    const result = await transform(inputData);
    return result;
  `);
  const result = await exec({ inputData: 'hello world' }).run();
  if (result !== 'HELLO WORLD') throw new Error(`Got '${result}'`);
  return result;
});

// Test 9: Arrow function callback with async inside
await test('Arrow function with async call inside', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      processItem: async (item) => item * 10
    }
  });
  const exec = s.compileAsync(`
    const items = [1, 2, 3];
    const fn = async (item) => {
      const result = await processItem(item);
      return result;
    };
    const results = [];
    for (const item of items) {
      results.push(await fn(item));
    }
    return results;
  `);
  const result = await exec().run();
  if (JSON.stringify(result) !== '[10,20,30]') throw new Error(`Got ${JSON.stringify(result)}`);
  return result;
});

// Test 10: map() as a custom global with concurrency
await test('Custom map() with concurrency control', async () => {
  let concurrent = 0;
  let maxConcurrent = 0;

  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      processItem: async (item) => {
        concurrent++;
        maxConcurrent = Math.max(maxConcurrent, concurrent);
        await new Promise(r => setTimeout(r, 50)); // simulate work
        concurrent--;
        return item * 2;
      },
      map: async (items, fn) => {
        const concurrency = 3;
        const results = [];
        const executing = new Set();
        for (const item of items) {
          const p = fn(item).then(result => {
            executing.delete(p);
            return result;
          });
          executing.add(p);
          results.push(p);
          if (executing.size >= concurrency) {
            await Promise.race(executing);
          }
        }
        return Promise.all(results);
      }
    }
  });
  const exec = s.compileAsync(`
    const items = [1, 2, 3, 4, 5, 6, 7, 8];
    const results = await map(items, async (item) => {
      return await processItem(item);
    });
    return results;
  `);
  const result = await exec().run();
  const expected = [2, 4, 6, 8, 10, 12, 14, 16];
  if (JSON.stringify(result) !== JSON.stringify(expected)) throw new Error(`Got ${JSON.stringify(result)}`);
  return { result, maxConcurrent };
});

// Test 11: map() called WITHOUT async/await in the callback (what LLM would write)
await test('map() where LLM writes sync-looking callback', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      processItem: async (item) => item * 2,
      map: async (items, fn) => {
        // fn might return a promise even if not declared async
        const results = [];
        for (const item of items) {
          const result = await fn(item);
          results.push(result);
        }
        return results;
      }
    }
  });
  // LLM writes this - no async, no await in callback
  const exec = s.compileAsync(`
    const items = [1, 2, 3];
    const results = await map(items, (item) => {
      return processItem(item);
    });
    return results;
  `);
  const result = await exec().run();
  if (JSON.stringify(result) !== '[2,4,6]') throw new Error(`Got ${JSON.stringify(result)}`);
  return result;
});

// Test 12: Verify blocked globals are truly inaccessible
await test('Blocked globals not accessible', async () => {
  const s = new Sandbox({
    globals: {
      ...Sandbox.SAFE_GLOBALS,
      // Deliberately NOT including: require, process, setTimeout, fetch
    }
  });
  const exec = s.compileAsync(`
    try {
      const x = setTimeout;
      return "FAIL: setTimeout accessible";
    } catch(e) {
      return "PASS: setTimeout blocked";
    }
  `);
  const result = await exec().run();
  return result;
});

console.log('\n--- Experiment complete ---');
