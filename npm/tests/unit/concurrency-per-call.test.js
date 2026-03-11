/**
 * Tests for per-LLM-call concurrency limiting (issue #512).
 *
 * Verifies that _wrapModelWithLimiter acquires/releases the concurrency slot
 * around each individual doStream/doGenerate call, NOT around the entire
 * multi-step agent session.
 */

import { describe, test, expect, beforeEach } from '@jest/globals';

// Import ProbeAgent to access the static _wrapModelWithLimiter method
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

/**
 * Create a mock limiter that tracks acquire/release calls with timestamps.
 */
function createMockLimiter(maxConcurrent = 3) {
  let active = 0;
  let totalAcquires = 0;
  let totalReleases = 0;
  const events = [];
  const waiters = [];

  return {
    async acquire(sessionId) {
      if (active >= maxConcurrent) {
        // Queue until a slot is available
        await new Promise(resolve => waiters.push(resolve));
      }
      active++;
      totalAcquires++;
      events.push({ type: 'acquire', time: Date.now(), active });
    },
    release(sessionId) {
      active--;
      totalReleases++;
      events.push({ type: 'release', time: Date.now(), active });
      if (waiters.length > 0) {
        const waiter = waiters.shift();
        waiter();
      }
    },
    getStats() {
      return {
        globalActive: active,
        maxConcurrent,
        queueSize: waiters.length
      };
    },
    // Test helpers
    get active() { return active; },
    get totalAcquires() { return totalAcquires; },
    get totalReleases() { return totalReleases; },
    get events() { return events; }
  };
}

/**
 * Create a mock model that simulates LLM doStream/doGenerate behavior.
 * Returns a ReadableStream that emits chunks, simulating streaming response.
 */
function createMockModel(options = {}) {
  const { streamChunks = ['Hello', ' ', 'world'], streamDelay = 0, generateResult = { text: 'result' }, throwOnStream = false, throwOnGenerate = false } = options;

  return {
    specificationVersion: 'v1',
    provider: 'test',
    modelId: 'test-model',
    defaultObjectGenerationMode: undefined,

    async doStream(callOptions) {
      if (throwOnStream) throw new Error('Stream error');

      const chunks = [...streamChunks];
      const stream = new ReadableStream({
        async pull(controller) {
          if (streamDelay > 0) {
            await new Promise(r => setTimeout(r, streamDelay));
          }
          if (chunks.length === 0) {
            controller.close();
          } else {
            controller.enqueue({ type: 'text-delta', textDelta: chunks.shift() });
          }
        }
      });

      return {
        stream,
        rawCall: { rawPrompt: null, rawSettings: {} },
        warnings: []
      };
    },

    async doGenerate(callOptions) {
      if (throwOnGenerate) throw new Error('Generate error');
      return generateResult;
    }
  };
}

/**
 * Consume a ReadableStream fully and return all chunks.
 */
async function consumeStream(stream) {
  const reader = stream.getReader();
  const chunks = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  return chunks;
}

describe('Per-LLM-call concurrency limiter — issue #512', () => {
  let limiter;

  beforeEach(() => {
    limiter = createMockLimiter(3);
  });

  describe('_wrapModelWithLimiter — doStream', () => {
    test('should acquire before and release after doStream completes', async () => {
      const model = createMockModel({ streamChunks: ['a', 'b', 'c'] });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      expect(limiter.active).toBe(0);

      const result = await wrapped.doStream({});
      // Slot should be acquired after doStream returns
      expect(limiter.active).toBe(1);

      // Consume the stream
      await consumeStream(result.stream);

      // Slot should be released after stream completes
      expect(limiter.active).toBe(0);
      expect(limiter.totalAcquires).toBe(1);
      expect(limiter.totalReleases).toBe(1);
    });

    test('should release slot when stream errors', async () => {
      const model = createMockModel();
      // Override doStream to return a stream that errors mid-way
      model.doStream = async () => {
        let count = 0;
        const stream = new ReadableStream({
          pull(controller) {
            if (count++ === 0) {
              controller.enqueue({ type: 'text-delta', textDelta: 'ok' });
            } else {
              controller.error(new Error('mid-stream error'));
            }
          }
        });
        return { stream, rawCall: { rawPrompt: null, rawSettings: {} } };
      };

      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);
      const result = await wrapped.doStream({});

      const reader = result.stream.getReader();
      // Read first chunk (ok)
      await reader.read();
      // Read second — should error
      try {
        await reader.read();
      } catch {
        // Expected
      }

      expect(limiter.active).toBe(0);
      expect(limiter.totalReleases).toBe(1);
    });

    test('should release slot when stream is cancelled', async () => {
      const model = createMockModel({ streamChunks: ['a', 'b', 'c', 'd', 'e'], streamDelay: 10 });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      const result = await wrapped.doStream({});
      expect(limiter.active).toBe(1);

      // Cancel the stream before fully consuming it
      const reader = result.stream.getReader();
      await reader.read(); // read one chunk
      await reader.cancel();

      expect(limiter.active).toBe(0);
      expect(limiter.totalReleases).toBe(1);
    });

    test('should release slot when doStream throws', async () => {
      const model = createMockModel({ throwOnStream: true });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      await expect(wrapped.doStream({})).rejects.toThrow('Stream error');

      expect(limiter.active).toBe(0);
      expect(limiter.totalAcquires).toBe(1);
      expect(limiter.totalReleases).toBe(1);
    });

    test('should preserve all other stream result properties', async () => {
      const model = createMockModel();
      model.doStream = async () => ({
        stream: new ReadableStream({ pull(c) { c.close(); } }),
        rawCall: { rawPrompt: 'test prompt', rawSettings: { temperature: 0.5 } },
        rawResponse: { headers: { 'x-test': '1' } },
        warnings: [{ type: 'unsupported-setting', setting: 'foo' }]
      });

      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);
      const result = await wrapped.doStream({});

      expect(result.rawCall.rawPrompt).toBe('test prompt');
      expect(result.rawCall.rawSettings.temperature).toBe(0.5);
      expect(result.rawResponse.headers['x-test']).toBe('1');
      expect(result.warnings).toHaveLength(1);

      await consumeStream(result.stream);
    });
  });

  describe('_wrapModelWithLimiter — doGenerate', () => {
    test('should acquire before and release after doGenerate completes', async () => {
      const model = createMockModel({ generateResult: { text: 'hello' } });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      const result = await wrapped.doGenerate({});

      expect(result.text).toBe('hello');
      expect(limiter.active).toBe(0);
      expect(limiter.totalAcquires).toBe(1);
      expect(limiter.totalReleases).toBe(1);
    });

    test('should release slot when doGenerate throws', async () => {
      const model = createMockModel({ throwOnGenerate: true });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      await expect(wrapped.doGenerate({})).rejects.toThrow('Generate error');

      expect(limiter.active).toBe(0);
      expect(limiter.totalAcquires).toBe(1);
      expect(limiter.totalReleases).toBe(1);
    });
  });

  describe('_wrapModelWithLimiter — model property passthrough', () => {
    test('should preserve specificationVersion', () => {
      const model = createMockModel();
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);
      expect(wrapped.specificationVersion).toBe('v1');
    });

    test('should preserve provider and modelId', () => {
      const model = createMockModel();
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);
      expect(wrapped.provider).toBe('test');
      expect(wrapped.modelId).toBe('test-model');
    });

    test('should preserve custom methods bound to original model', () => {
      const model = createMockModel();
      model.customMethod = function() { return this.modelId; };
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);
      expect(wrapped.customMethod()).toBe('test-model');
    });
  });

  describe('per-call behavior — multi-step simulation', () => {
    test('slot is released between consecutive doStream calls (simulating steps)', async () => {
      const model = createMockModel({ streamChunks: ['step-data'] });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      // Simulate 3 steps of an agent loop
      for (let step = 0; step < 3; step++) {
        // Before each step, slot should be free
        expect(limiter.active).toBe(0);

        // doStream acquires the slot
        const result = await wrapped.doStream({});
        expect(limiter.active).toBe(1);

        // Consume stream (simulates reading LLM response)
        await consumeStream(result.stream);

        // After consuming, slot should be released
        expect(limiter.active).toBe(0);

        // Simulate tool execution time between steps (no slot held)
        // In production, tools like bash/search run here
      }

      expect(limiter.totalAcquires).toBe(3);
      expect(limiter.totalReleases).toBe(3);
    });

    test('concurrent sessions can interleave LLM calls', async () => {
      // maxConcurrent=2, 3 sessions
      const limiter2 = createMockLimiter(2);
      const model = createMockModel({ streamChunks: ['data'], streamDelay: 5 });

      const wrapped1 = ProbeAgent._wrapModelWithLimiter(model, limiter2, false);
      const wrapped2 = ProbeAgent._wrapModelWithLimiter(model, limiter2, false);
      const wrapped3 = ProbeAgent._wrapModelWithLimiter(model, limiter2, false);

      // Session 1 & 2 start streaming (fill 2 slots)
      const result1 = await wrapped1.doStream({});
      const result2 = await wrapped2.doStream({});
      expect(limiter2.active).toBe(2);

      // Session 3 should queue
      let session3Started = false;
      const session3Promise = wrapped3.doStream({}).then(r => {
        session3Started = true;
        return r;
      });

      await new Promise(r => setTimeout(r, 10));
      expect(session3Started).toBe(false);

      // Session 1 finishes streaming → releases slot → session 3 acquires it
      await consumeStream(result1.stream);

      // Session 3 should now get the slot (session 2 + session 3 = 2 active)
      const result3 = await session3Promise;
      expect(session3Started).toBe(true);
      expect(limiter2.active).toBe(2); // session 2 + session 3

      // Clean up remaining streams
      await consumeStream(result2.stream);
      expect(limiter2.active).toBe(1); // only session 3
      await consumeStream(result3.stream);
      expect(limiter2.active).toBe(0);
    });

    test('tool execution does not hold a slot', async () => {
      const model = createMockModel({ streamChunks: ['response'] });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      // Step 1: LLM call
      const result1 = await wrapped.doStream({});
      expect(limiter.active).toBe(1);
      await consumeStream(result1.stream);
      expect(limiter.active).toBe(0);

      // Simulate tool execution (50ms)
      // During this time, the slot is FREE for other sessions
      const slotDuringTool = limiter.active;
      await new Promise(r => setTimeout(r, 50));
      expect(slotDuringTool).toBe(0);

      // Step 2: Next LLM call
      const result2 = await wrapped.doStream({});
      expect(limiter.active).toBe(1);
      await consumeStream(result2.stream);
      expect(limiter.active).toBe(0);

      // Verify: 2 acquire/release cycles, not 1 long session hold
      expect(limiter.totalAcquires).toBe(2);
      expect(limiter.totalReleases).toBe(2);
    });
  });

  describe('debug logging', () => {
    test('should log acquire/release when debug=true', async () => {
      const logs = [];
      const origLog = console.log;
      console.log = (...args) => logs.push(args.join(' '));

      try {
        const model = createMockModel({ streamChunks: ['x'] });
        const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, true);

        const result = await wrapped.doStream({});
        await consumeStream(result.stream);

        expect(logs.some(l => l.includes('Acquired AI slot'))).toBe(true);
        expect(logs.some(l => l.includes('Released AI slot'))).toBe(true);
      } finally {
        console.log = origLog;
      }
    });

    test('should not log when debug=false', async () => {
      const logs = [];
      const origLog = console.log;
      console.log = (...args) => logs.push(args.join(' '));

      try {
        const model = createMockModel({ streamChunks: ['x'] });
        const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

        const result = await wrapped.doStream({});
        await consumeStream(result.stream);

        expect(logs.filter(l => l.includes('AI slot')).length).toBe(0);
      } finally {
        console.log = origLog;
      }
    });
  });

  describe('edge cases', () => {
    test('empty stream should still release slot', async () => {
      const model = createMockModel({ streamChunks: [] });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      const result = await wrapped.doStream({});
      expect(limiter.active).toBe(1);

      await consumeStream(result.stream);
      expect(limiter.active).toBe(0);
    });

    test('single-chunk stream should acquire and release correctly', async () => {
      const model = createMockModel({ streamChunks: ['only-chunk'] });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      const result = await wrapped.doStream({});
      const chunks = await consumeStream(result.stream);

      expect(chunks).toHaveLength(1);
      expect(limiter.active).toBe(0);
    });

    test('large number of sequential calls should not leak slots', async () => {
      const model = createMockModel({ streamChunks: ['x'] });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      for (let i = 0; i < 50; i++) {
        const result = await wrapped.doStream({});
        await consumeStream(result.stream);
      }

      expect(limiter.active).toBe(0);
      expect(limiter.totalAcquires).toBe(50);
      expect(limiter.totalReleases).toBe(50);
    });

    test('mixed doStream and doGenerate calls work correctly', async () => {
      const model = createMockModel({
        streamChunks: ['streamed'],
        generateResult: { text: 'generated' }
      });
      const wrapped = ProbeAgent._wrapModelWithLimiter(model, limiter, false);

      // doStream
      const streamResult = await wrapped.doStream({});
      await consumeStream(streamResult.stream);
      expect(limiter.active).toBe(0);

      // doGenerate
      const genResult = await wrapped.doGenerate({});
      expect(genResult.text).toBe('generated');
      expect(limiter.active).toBe(0);

      // Another doStream
      const streamResult2 = await wrapped.doStream({});
      await consumeStream(streamResult2.stream);
      expect(limiter.active).toBe(0);

      expect(limiter.totalAcquires).toBe(3);
      expect(limiter.totalReleases).toBe(3);
    });
  });
});
