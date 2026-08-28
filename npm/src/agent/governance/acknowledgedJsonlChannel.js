/**
 * Internal acknowledged JSONL channel.
 *
 * Semantics are derived from the accepted ReqProof EXP-0171 cooperative
 * channel at commit dcc888120cfbeb04f8bfe59147f272c2396723e0. This is the
 * reusable channel primitive only, not a port of that experiment's runner.
 */

import { PassThrough } from 'stream';

const DEFAULT_FRAME_BYTE_CAP = 256;
const DEFAULT_TOTAL_BYTE_CAP = 1024;
const DEFAULT_IDLE_TIMEOUT_MS = 50;
const DEFAULT_DEADLINE_MS = 400;
const DEFAULT_HIGH_WATER_MARK = 16 * 1024;

function positiveInteger(value, fallback, name) {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new TypeError(`${name} must be a positive safe integer`);
  }
  return value;
}

function nonNegativeNumber(value, fallback, name) {
  if (value === undefined) return fallback;
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) {
    throw new TypeError(`${name} must be a non-negative finite number`);
  }
  return value;
}

export function createAcknowledgedJsonlChannel(options = {}) {
  if (typeof options.onRecord !== 'function') {
    throw new TypeError('onRecord must be a function');
  }

  const frameByteCap = positiveInteger(options.frameByteCap, DEFAULT_FRAME_BYTE_CAP, 'frameByteCap');
  const totalByteCap = positiveInteger(options.totalByteCap, DEFAULT_TOTAL_BYTE_CAP, 'totalByteCap');
  const idleTimeoutMs = nonNegativeNumber(options.idleTimeoutMs, DEFAULT_IDLE_TIMEOUT_MS, 'idleTimeoutMs');
  const deadlineMs = nonNegativeNumber(options.deadlineMs, DEFAULT_DEADLINE_MS, 'deadlineMs');
  const highWaterMark = positiveInteger(options.highWaterMark, DEFAULT_HIGH_WATER_MARK, 'highWaterMark');

  const stream = new PassThrough({ highWaterMark });
  const controller = new AbortController();
  const pendingAcks = new Set();
  const pendingWrites = new Set();
  const timers = new Set();
  let partial = Buffer.alloc(0);
  let totalBytes = 0;
  let frames = 0;
  let acknowledgements = 0;
  let accepting = true;
  let eof = false;
  let settled = false;
  let cleaned = false;
  let cleanupPromise = null;
  let failure = null;
  let firstFailureCount = 0;
  let laterFailureCount = 0;
  let abortCount = 0;
  let backpressureCount = 0;
  let drainWaiters = 0;
  let idleTimer = null;
  let deadlineTimer = null;
  let resolveResult;

  const result = new Promise(resolve => { resolveResult = resolve; });
  const closed = new Promise(resolve => stream.once('close', resolve));

  const clearOwnedTimer = timer => {
    if (!timer) return;
    clearTimeout(timer);
    timers.delete(timer);
  };

  const ownedTimer = (delay, callback) => {
    if (delay === 0) return null;
    const timer = setTimeout(() => {
      timers.delete(timer);
      callback();
    }, delay);
    timers.add(timer);
    return timer;
  };

  const clearTimers = () => {
    clearOwnedTimer(idleTimer);
    clearOwnedTimer(deadlineTimer);
    idleTimer = null;
    deadlineTimer = null;
  };

  const settle = classification => {
    if (settled) return;
    settled = true;
    accepting = false;
    clearTimers();
    resolveResult(Object.freeze({
      classification,
      error: failure?.message ?? null,
      frames,
      acknowledgements,
      eof
    }));
  };

  const fail = (classification, error) => {
    if (failure) {
      laterFailureCount += 1;
      return false;
    }
    failure = {
      classification,
      message: error instanceof Error ? error.message : String(error ?? classification)
    };
    firstFailureCount += 1;
    abortCount += 1;
    accepting = false;
    controller.abort(failure);
    settle(classification);
    return true;
  };

  const maybePass = () => {
    if (eof && pendingAcks.size === 0 && !failure) settle('PASS');
  };

  const touchIdle = () => {
    clearOwnedTimer(idleTimer);
    idleTimer = ownedTimer(idleTimeoutMs, () => fail('FAIL_IDLE_TIMEOUT'));
  };

  const observeAck = value => {
    const underlying = Promise.resolve(value);
    let active = true;
    let resolveOwned;
    const owned = new Promise(resolve => { resolveOwned = resolve; });
    const finish = acknowledged => {
      if (!active) return;
      active = false;
      controller.signal.removeEventListener('abort', onAbort);
      if (acknowledged) acknowledgements += 1;
      resolveOwned();
    };
    const onAbort = () => finish(false);

    pendingAcks.add(owned);
    controller.signal.addEventListener('abort', onAbort, { once: true });
    underlying.then(
      () => finish(true),
      error => {
        fail('FAIL_ACK', error);
        finish(false);
      }
    );
    if (controller.signal.aborted) onAbort();
    owned.then(() => {
      pendingAcks.delete(owned);
      maybePass();
    });
  };

  const acceptFrame = bytes => {
    let record;
    try {
      record = JSON.parse(bytes.toString('utf8'));
    } catch (error) {
      fail('FAIL_PARSE', error);
      return;
    }
    const keys = record && typeof record === 'object' ? Object.keys(record) : [];
    if (keys.join(',') !== 'id,value' ||
        !Number.isSafeInteger(record.id) || record.id < 0 || typeof record.value !== 'string' ||
        !bytes.equals(Buffer.from(JSON.stringify(record), 'utf8'))) {
      fail('FAIL_PARSE', 'frame is not canonical JSON');
      return;
    }
    frames += 1;
    try {
      observeAck(options.onRecord(record, controller.signal));
    } catch (error) {
      fail('FAIL_ACK', error);
    }
  };

  const ingest = chunk => {
    if (!accepting) return;
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    let offset = 0;
    while (offset < bytes.length && accepting) {
      const newline = bytes.indexOf(10, offset);
      const end = newline === -1 ? bytes.length : newline;
      const segment = bytes.subarray(offset, end);
      if (partial.length + segment.length > frameByteCap) {
        fail('FAIL_FRAME_CAP');
        return;
      }
      if (segment.length > 0) partial = Buffer.concat([partial, segment]);
      if (newline === -1) break;
      const frame = partial;
      partial = Buffer.alloc(0);
      acceptFrame(frame);
      offset = newline + 1;
    }
    if (accepting) touchIdle();
  };

  const onData = chunk => {
    try { ingest(chunk); } catch (error) { fail('FAIL_SOURCE', error); }
  };
  const onEnd = () => {
    eof = true;
    clearOwnedTimer(idleTimer);
    idleTimer = null;
    if (partial.length > 0) fail('FAIL_PARTIAL_EOF');
    else maybePass();
  };
  const onError = error => fail('FAIL_SOURCE', error);

  stream.on('data', onData);
  stream.once('end', onEnd);
  stream.on('error', onError);
  touchIdle();
  deadlineTimer = ownedTimer(deadlineMs, () => fail('FAIL_DEADLINE'));

  const trackWrite = operation => {
    pendingWrites.add(operation);
    operation.finally(() => pendingWrites.delete(operation));
    return operation;
  };

  const write = input => {
    if (!accepting) return Promise.resolve(false);
    let byteLength;
    if (Buffer.isBuffer(input) || input instanceof Uint8Array) byteLength = input.byteLength;
    else if (typeof input === 'string') byteLength = Buffer.byteLength(input);
    else throw new TypeError('write input must be a Buffer, string, or Uint8Array');
    if (totalBytes + byteLength > totalByteCap) {
      fail('FAIL_TOTAL_CAP');
      return Promise.resolve(false);
    }
    const bytes = Buffer.from(input);
    totalBytes += byteLength;
    let resolveCallback;
    const callbackDone = new Promise(resolve => { resolveCallback = resolve; });
    let resolveDrain = () => {};
    let drained = null;

    stream.cork();
    const accepted = stream.write(bytes, error => {
      if (error) fail('FAIL_SOURCE', error);
      resolveCallback();
    });
    if (!accepted) {
      backpressureCount += 1;
      drainWaiters += 1;
      drained = new Promise(resolve => {
        resolveDrain = () => {
          stream.off('drain', onDrain);
          controller.signal.removeEventListener('abort', onAbort);
          drainWaiters -= 1;
          resolve();
        };
        const onDrain = () => resolveDrain();
        const onAbort = () => resolveDrain();
        stream.once('drain', onDrain);
        controller.signal.addEventListener('abort', onAbort, { once: true });
      });
    }
    queueMicrotask(() => stream.uncork());

    return trackWrite(Promise.all([callbackDone, drained ?? Promise.resolve()])
      .then(() => accepting || eof));
  };

  const end = () => {
    if (stream.writableEnded || stream.destroyed) return Promise.resolve();
    let resolveEnd;
    const operation = new Promise(resolve => { resolveEnd = resolve; });
    pendingWrites.add(operation);
    stream.end(() => resolveEnd());
    operation.finally(() => pendingWrites.delete(operation));
    return operation;
  };

  const snapshot = () => Object.freeze({
    accepting,
    eof,
    frames,
    acknowledgements,
    totalBytes,
    partialBytes: partial.length,
    pending: pendingAcks.size,
    writes: pendingWrites.size,
    timers: timers.size,
    drainWaiters,
    backpressureCount,
    firstFailureCount,
    laterFailureCount,
    abortCount,
    listeners: ['data', 'end', 'error', 'close', 'drain']
      .reduce((count, event) => count + stream.listenerCount(event), 0)
  });

  const cleanup = () => {
    if (cleanupPromise) return cleanupPromise;
    cleanupPromise = (async () => {
      clearTimers();
      if (pendingAcks.size > 0 && !failure) fail('FAIL_CLEANUP');
      if (!stream.writableEnded && !stream.destroyed) {
        if (failure) stream.destroy();
        else await end();
      }
      await Promise.allSettled([...pendingWrites]);
      await Promise.allSettled([...pendingAcks]);
      if (!stream.destroyed) stream.destroy();
      await closed;
      stream.off('data', onData);
      stream.off('end', onEnd);
      stream.off('error', onError);
      cleaned = true;
    })();
    return cleanupPromise;
  };

  return Object.freeze({ write, end, result, cleanup, snapshot: () => ({ ...snapshot(), cleaned }) });
}
