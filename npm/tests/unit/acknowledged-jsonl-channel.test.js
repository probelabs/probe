import { describe, expect, jest, test } from '@jest/globals';
import { createAcknowledgedJsonlChannel } from '../../src/agent/governance/acknowledgedJsonlChannel.js';

const encode = record => Buffer.from(`${JSON.stringify(record)}\n`);

describe('createAcknowledgedJsonlChannel', () => {
  test('parses fragmented canonical JSONL', async () => {
    const records = [];
    const channel = createAcknowledgedJsonlChannel({
      onRecord: async record => { records.push(record); },
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });
    const frame = encode({ id: 1, value: 'fragmented' });

    await channel.write(frame.subarray(0, 3));
    await channel.write(frame.subarray(3, 11));
    await channel.write(frame.subarray(11));
    await channel.end();

    expect(await channel.result).toMatchObject({ classification: 'PASS', frames: 1, acknowledgements: 1, eof: true });
    expect(records).toEqual([{ id: 1, value: 'fragmented' }]);
    await channel.cleanup();
    expect(channel.snapshot()).toMatchObject({ pending: 0, writes: 0, timers: 0, listeners: 0, cleaned: true });
  });

  test('requires the exact canonical id/value schema', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => undefined,
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });

    await channel.write(`${JSON.stringify({ value: 'wrong-order', id: 1 })}\n`);
    expect(await channel.result).toMatchObject({ classification: 'FAIL_PARSE', frames: 0, acknowledgements: 0 });
    await channel.cleanup();
  });

  test('owns forced cork backpressure through callback and drain', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => undefined,
      frameByteCap: 1024,
      totalByteCap: 2048,
      highWaterMark: 1,
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });

    await channel.write(encode({ id: 1, value: 'x'.repeat(300) }));
    expect(channel.snapshot()).toMatchObject({ backpressureCount: 1, drainWaiters: 0, writes: 0 });
    await channel.end();
    expect((await channel.result).classification).toBe('PASS');
    await channel.cleanup();
  });

  test('does not pass EOF until the asynchronous ACK settles', async () => {
    let releaseAck;
    const ack = new Promise(resolve => { releaseAck = resolve; });
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => ack,
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });

    await channel.write(encode({ id: 1, value: 'await-ack' }));
    await channel.end();
    const beforeAck = await Promise.race([
      channel.result.then(() => 'settled'),
      new Promise(resolve => setImmediate(() => resolve('pending')))
    ]);
    expect(beforeAck).toBe('pending');
    expect(channel.snapshot()).toMatchObject({ eof: true, pending: 1, acknowledgements: 0 });

    releaseAck();
    expect(await channel.result).toMatchObject({ classification: 'PASS', acknowledgements: 1 });
    await channel.cleanup();
  });

  test('aborts on the first failure and cleanup drains owned state', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => undefined,
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });

    await channel.write(Buffer.from('{bad}\n'));
    await channel.write(Buffer.alloc(512, 97));
    expect(await channel.result).toMatchObject({ classification: 'FAIL_PARSE' });
    expect(channel.snapshot()).toMatchObject({ firstFailureCount: 1, abortCount: 1 });

    await channel.cleanup();
    expect(channel.snapshot()).toMatchObject({ pending: 0, writes: 0, timers: 0, drainWaiters: 0, listeners: 0, cleaned: true });
  });

  test.each([
    ['frame cap', { frameByteCap: 8, totalByteCap: 1024 }, '123456789', false, 'FAIL_FRAME_CAP'],
    ['partial EOF', { frameByteCap: 256, totalByteCap: 1024 }, '{"id":1', true, 'FAIL_PARTIAL_EOF']
  ])('classifies %s and cleans owned state', async (_name, limits, input, shouldEnd, classification) => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => undefined,
      ...limits,
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });
    await channel.write(input);
    if (shouldEnd) await channel.end();
    expect((await channel.result).classification).toBe(classification);
    await channel.cleanup();
    expect(channel.snapshot()).toMatchObject({ pending: 0, writes: 0, timers: 0, listeners: 0, cleaned: true });
  });

  test('rejects total overflow before converting or allocating the huge input', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => undefined,
      totalByteCap: 16,
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });
    const fromSpy = jest.spyOn(Buffer, 'from');
    try {
      await channel.write('x'.repeat(1024 * 1024));
      expect(fromSpy).not.toHaveBeenCalled();
    } finally {
      fromSpy.mockRestore();
    }
    expect((await channel.result).classification).toBe('FAIL_TOTAL_CAP');
    await channel.cleanup();
  });

  test('idle timeout aborts a never-settling ACK and cleanup reaches zero', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => new Promise(() => {}),
      idleTimeoutMs: 20,
      deadlineMs: 1000
    });
    await channel.write(encode({ id: 1, value: 'never' }));
    expect((await channel.result).classification).toBe('FAIL_IDLE_TIMEOUT');
    await channel.cleanup();
    expect(channel.snapshot()).toMatchObject({ pending: 0, timers: 0, listeners: 0, abortCount: 1, cleaned: true });
  });

  test('deadline aborts a never-settling ACK after EOF and cleanup reaches zero', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => new Promise(() => {}),
      idleTimeoutMs: 0,
      deadlineMs: 20
    });
    await channel.write(encode({ id: 1, value: 'never' }));
    await channel.end();
    expect((await channel.result).classification).toBe('FAIL_DEADLINE');
    await channel.cleanup();
    expect(channel.snapshot()).toMatchObject({ pending: 0, timers: 0, listeners: 0, abortCount: 1, cleaned: true });
  });

  test('keeps the first failure immutable while observing a competing late rejection', async () => {
    const channel = createAcknowledgedJsonlChannel({
      onRecord: () => new Promise((_, reject) => setImmediate(() => reject(new Error('late ACK failure')))),
      idleTimeoutMs: 1000,
      deadlineMs: 2000
    });
    await channel.write(encode({ id: 1, value: 'will-reject' }));
    await channel.write('{bad}\n');
    expect((await channel.result).classification).toBe('FAIL_PARSE');
    await new Promise(resolve => setImmediate(resolve));
    await channel.cleanup();
    expect(channel.snapshot()).toMatchObject({
      firstFailureCount: 1,
      laterFailureCount: 1,
      abortCount: 1,
      pending: 0,
      timers: 0,
      listeners: 0
    });
  });
});
