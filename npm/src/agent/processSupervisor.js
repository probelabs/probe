/**
 * Governed child-process execution with bounded output and explicit lifecycle facts.
 * The ChildProcess and its PID intentionally never cross this module boundary.
 * @module agent/processSupervisor
 */

import { spawn } from 'child_process';
import { randomBytes } from 'crypto';

const DEFAULT_TERMINATION_GRACE_MS = 5000;
const DEFAULT_CLEANUP_TIMEOUT_MS = 10000;
const DEFAULT_STREAM_BYTE_CAP = 10 * 1024 * 1024;

let nextProcessSequence = 0;

function makeId() {
  nextProcessSequence += 1;
  return `process-${nextProcessSequence.toString(36)}-${randomBytes(6).toString('hex')}`;
}

function nonNegativeNumber(value, fallback, name) {
  if (value === undefined) return fallback;
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) {
    throw new TypeError(`${name} must be a non-negative finite number`);
  }
  return value;
}

function byteCap(value, fallback, name) {
  const cap = nonNegativeNumber(value, fallback, name);
  if (!Number.isSafeInteger(cap)) {
    throw new TypeError(`${name} must be a safe integer`);
  }
  return cap;
}

function makeSettledHandle(id, error) {
  const message = error instanceof Error ? error.message : String(error);
  const receipt = Object.freeze({
    id,
    classification: 'spawn_error',
    reason: 'spawn_error',
    error: message,
    stdout: '',
    stderr: '',
    stdoutBytes: 0,
    stderrBytes: 0,
    exitCode: null,
    signal: null,
    barriers: Object.freeze({ close: false, stdoutEOF: false, stderrEOF: false }),
    observed: Object.freeze([Object.freeze({ sequence: 1, fact: 'spawn-error', error: message })])
  });
  const result = Promise.resolve(receipt);
  return Object.freeze({ id, terminate: () => result, result });
}

/**
 * Spawn a process whose resources and termination are governed by fixed deadlines.
 *
 * @param {Object} spec
 * @param {string} spec.command
 * @param {string[]} [spec.args]
 * @param {string} [spec.cwd]
 * @param {NodeJS.ProcessEnv} [spec.env]
 * @param {AbortSignal} [spec.signal]
 * @param {number} [spec.executionTimeoutMs=0]
 * @param {number} [spec.terminationGraceMs=5000]
 * @param {number} [spec.cleanupTimeoutMs=10000] Must be at least terminationGraceMs.
 * @param {number} [spec.stdoutByteCap=10485760]
 * @param {number} [spec.stderrByteCap=10485760]
 * @param {'child'|'process-group'} [spec.signalScope='child']
 * @returns {{id: string, terminate: (reason?: string) => Promise<Object>, result: Promise<Object>}}
 */
function governProcess(spec, attachedChild = null) {
  if (!spec || typeof spec !== 'object') {
    throw new TypeError('spec must be an object');
  }
  if (typeof spec.command !== 'string' || spec.command.length === 0) {
    throw new TypeError('command must be a non-empty string');
  }
  if (spec.args !== undefined && (!Array.isArray(spec.args) || spec.args.some(arg => typeof arg !== 'string'))) {
    throw new TypeError('args must be an array of strings');
  }
  if (spec.signal !== undefined && (
    !spec.signal ||
    typeof spec.signal !== 'object' ||
    typeof spec.signal.aborted !== 'boolean' ||
    typeof spec.signal.addEventListener !== 'function' ||
    typeof spec.signal.removeEventListener !== 'function'
  )) {
    throw new TypeError('signal must be an AbortSignal');
  }

  const executionTimeoutMs = nonNegativeNumber(spec.executionTimeoutMs, 0, 'executionTimeoutMs');
  const terminationGraceMs = nonNegativeNumber(spec.terminationGraceMs, DEFAULT_TERMINATION_GRACE_MS, 'terminationGraceMs');
  const cleanupTimeoutMs = nonNegativeNumber(spec.cleanupTimeoutMs, DEFAULT_CLEANUP_TIMEOUT_MS, 'cleanupTimeoutMs');
  if (cleanupTimeoutMs < terminationGraceMs) {
    throw new TypeError('cleanupTimeoutMs must be greater than or equal to terminationGraceMs');
  }
  const stdoutByteCap = byteCap(spec.stdoutByteCap, DEFAULT_STREAM_BYTE_CAP, 'stdoutByteCap');
  const stderrByteCap = byteCap(spec.stderrByteCap, DEFAULT_STREAM_BYTE_CAP, 'stderrByteCap');
  const captureStdout = !attachedChild || spec.captureStdout !== false;
  const signalScope = spec.signalScope ?? 'child';
  if (signalScope !== 'child' && signalScope !== 'process-group') {
    throw new TypeError("signalScope must be 'child' or 'process-group'");
  }

  const id = makeId();
  let child;
  if (attachedChild) {
    child = attachedChild;
  } else {
    try {
      child = spawn(spec.command, spec.args ?? [], {
        cwd: spec.cwd,
        env: spec.env,
        stdio: ['ignore', 'pipe', 'pipe'],
        shell: false,
        detached: signalScope === 'process-group',
        windowsHide: true
      });
    } catch (error) {
      return makeSettledHandle(id, error);
    }
  }

  let resolveResult;
  const result = new Promise(resolve => { resolveResult = resolve; });
  const observed = [];
  const barriers = { close: false, stdoutEOF: false, stderrEOF: false };
  const stdoutChunks = [];
  const stderrChunks = [];
  let stdoutBytes = 0;
  let stderrBytes = 0;
  let exitCode = null;
  let exitSignal = null;
  let exitObserved = false;
  let settled = false;
  let terminalReason = null;
  let terminalClassification = null;
  let terminalError = null;
  let lastSignalDelivery = null;
  let executionTimer = null;
  let escalationTimer = null;
  let cleanupTimer = null;

  const observe = (fact, details = {}) => {
    if (settled) return;
    observed.push(Object.freeze({ sequence: observed.length + 1, fact, ...details }));
  };

  const startTimer = (callback, delay) => {
    const timer = setTimeout(callback, delay);
    timer.unref?.();
    return timer;
  };

  const clearTimer = (timer) => {
    if (timer) clearTimeout(timer);
  };

  const settle = (classification, reason = terminalReason) => {
    if (settled) return;
    settled = true;
    clearTimer(executionTimer);
    clearTimer(cleanupTimer);
    clearTimer(escalationTimer);
    if (spec.signal) spec.signal.removeEventListener('abort', onAbort);

    resolveResult(Object.freeze({
      id,
      classification,
      reason: reason ?? null,
      ...(terminalError ? { error: terminalError } : {}),
      stdout: Buffer.concat(stdoutChunks, stdoutBytes).toString(),
      stderr: Buffer.concat(stderrChunks, stderrBytes).toString(),
      stdoutBytes,
      stderrBytes,
      exitCode,
      signal: exitSignal,
      barriers: Object.freeze({ ...barriers }),
      observed: Object.freeze([...observed])
    }));
  };

  const fullBarrierObserved = () => barriers.close && barriers.stdoutEOF && barriers.stderrEOF;

  const maybeSettle = () => {
    if (settled || !exitObserved || !fullBarrierObserved()) return;
    settle(terminalClassification ?? 'exited', terminalReason);
  };

  const startCleanupDeadline = () => {
    if (cleanupTimer || settled) return;
    cleanupTimer = startTimer(() => {
      observe('barrier', { barrier: 'cleanup_deadline' });
      child.stdout.destroy();
      child.stderr.destroy();
      settle('cleanup_timeout', terminalReason ?? 'cleanup_timeout');
    }, cleanupTimeoutMs);
  };

  const attemptSignal = (signal) => {
    let accepted = false;
    if (settled || exitObserved) return;
    if (!child.pid) {
      observe('signal-attempt', {
        signal,
        requestedScope: signalScope,
        actualScope: null,
        accepted: false
      });
      return;
    }
    let actualScope = null;
    try {
      if (signalScope === 'process-group') {
        process.kill(-child.pid, signal);
        accepted = true;
        actualScope = 'process-group';
      } else {
        accepted = child.kill(signal);
        if (accepted) actualScope = 'child';
      }
    } catch {
      try {
        accepted = child.kill(signal);
        if (accepted) actualScope = 'child';
      } catch {
        accepted = false;
      }
    }
    observe('signal-attempt', {
      signal,
      requestedScope: signalScope,
      actualScope,
      accepted: Boolean(accepted)
    });
    if (accepted) lastSignalDelivery = { signal, actualScope };
  };

  const beginTermination = (reason, classification = 'terminated') => {
    if (settled || exitObserved || terminalClassification) return result;
    terminalReason = typeof reason === 'string' && reason.length > 0 ? reason : 'terminated';
    terminalClassification = classification;
    clearTimer(executionTimer);
    attemptSignal('SIGTERM');
    escalationTimer = startTimer(() => attemptSignal('SIGKILL'), terminationGraceMs);
    startCleanupDeadline();
    return result;
  };

  const appendChunk = (stream, data) => {
    if (settled) return;
    const chunk = Buffer.isBuffer(data) ? data : Buffer.from(data);
    const isStdout = stream === 'stdout';
    const used = isStdout ? stdoutBytes : stderrBytes;
    const cap = isStdout ? stdoutByteCap : stderrByteCap;
    const remaining = Math.max(0, cap - used);
    const kept = remaining >= chunk.length ? chunk : chunk.subarray(0, remaining);
    if (kept.length > 0) {
      (isStdout ? stdoutChunks : stderrChunks).push(kept);
      if (isStdout) stdoutBytes += kept.length;
      else stderrBytes += kept.length;
    }
    if (chunk.length > remaining) {
      beginTermination(`${stream}_overflow`, 'output_overflow');
    }
  };

  const observeBarrier = (barrier) => {
    if (settled || barriers[barrier]) return;
    barriers[barrier] = true;
    observe('barrier', { barrier });
    maybeSettle();
  };

  const onAbort = () => beginTermination('aborted', 'aborted');

  if (captureStdout) child.stdout.on('data', data => appendChunk('stdout', data));
  child.stderr.on('data', data => appendChunk('stderr', data));
  child.stdout.once('end', () => observeBarrier('stdoutEOF'));
  child.stderr.once('end', () => observeBarrier('stderrEOF'));

  child.once('exit', (code, signal) => {
    if (settled) return;
    exitObserved = true;
    clearTimer(executionTimer);
    clearTimer(escalationTimer);
    exitCode = code;
    exitSignal = signal;
    observe('exit', { code, signal });
    if (signal) {
      observe('signal', {
        signal,
        requestedScope: lastSignalDelivery?.signal === signal ? signalScope : null,
        actualScope: lastSignalDelivery?.signal === signal ? lastSignalDelivery.actualScope : null
      });
    }
    startCleanupDeadline();
    maybeSettle();
  });

  child.once('close', () => observeBarrier('close'));

  child.once('error', error => {
    if (settled) return;
    terminalError = error.message;
    observe('spawn-error', { error: error.message });
    settle('spawn_error', 'spawn_error');
  });

  if (executionTimeoutMs > 0) {
    executionTimer = startTimer(
      () => beginTermination('execution_timeout', 'execution_timeout'),
      executionTimeoutMs
    );
  }

  if (spec.signal) {
    spec.signal.addEventListener('abort', onAbort, { once: true });
    if (spec.signal.aborted) onAbort();
  }

  return Object.freeze({
    id,
    terminate: reason => beginTermination(reason),
    result
  });
}

export function spawnGovernedProcess(spec) {
  return governProcess(spec);
}

/**
 * Internal duplex adapter for engines that must retain protocol access to a child.
 * The returned handle keeps the same bounded termination and close/EOF barriers as
 * spawnGovernedProcess without exposing the child through the public governance API.
 *
 * @param {import('child_process').ChildProcess} child
 * @param {Object} [spec]
 * @returns {{id: string, terminate: (reason?: string) => Promise<Object>, result: Promise<Object>}}
 */
export function governSpawnedProcess(child, spec = {}) {
  if (!child || typeof child !== 'object' || typeof child.kill !== 'function' ||
      !child.stdout || !child.stderr) {
    throw new TypeError('child must be a spawned process with stdout and stderr pipes');
  }
  return governProcess({ ...spec, command: 'attached-child' }, child);
}
