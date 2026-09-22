import { describe, expect, test } from '@jest/globals';
import {
  existsSync,
  lstatSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmdirSync,
  unlinkSync,
  writeFileSync
} from 'fs';
import { tmpdir } from 'os';
import { basename, dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { createAcknowledgedJsonlChannel } from '../../src/agent/governance/acknowledgedJsonlChannel.js';
import { writeAtomicTerminalReceipt } from '../../src/agent/governance/atomicTerminalReceipt.js';
import { spawnGovernedProcess } from '../../src/agent/processSupervisor.js';

// Test-only five-case recombination: no live protocol, seven-case runner,
// public sibling API, coordinator, fanout, Proof, Visor, Luna, model, provider,
// network, or API behavior is introduced here. Receipt publication is observed
// process atomicity, not power-loss directory-entry durability.
const fixtureDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'fixtures', 'governed-siblings');
const CLEANUP_CHILDREN = [
  'worker-ready',
  'controller-decision.tmp',
  'controller-decision',
  'receipt.json.tmp',
  'receipt.json'
];

async function waitForFile(file, timeoutMs = 2000) {
  const deadline = Date.now() + timeoutMs;
  while (!existsSync(file)) {
    if (Date.now() >= deadline) throw new Error(`worker did not become ready: ${file}`);
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

function aggregateReceipt(receipt) {
  return {
    classification: receipt.classification,
    exitCode: receipt.exitCode,
    signal: receipt.signal,
    barriers: receipt.barriers,
    signalAttempts: receipt.observed
      .filter(event => event.fact === 'signal-attempt')
      .map(event => ({ signal: event.signal, accepted: event.accepted }))
  };
}

function canonicalReceiptBytes(observed) {
  return Buffer.from(`${JSON.stringify(observed)}\n`);
}

function lstatIfPresent(path) {
  try { return lstatSync(path); }
  catch (error) { if (error?.code === 'ENOENT') return null; throw error; }
}

function guardAttemptRoot(tempDir) {
  const root = lstatSync(tempDir);
  if (!root.isDirectory() || root.isSymbolicLink() || realpathSync(tempDir) !== tempDir) {
    throw new Error('sibling cleanup root lost its canonical nonsymlink boundary');
  }
}

function cleanupExactAttempt(tempDir) {
  guardAttemptRoot(tempDir);
  for (const name of CLEANUP_CHILDREN) {
    guardAttemptRoot(tempDir);
    const path = join(tempDir, name);
    if (dirname(path) !== tempDir || basename(path) !== name) throw new Error('invalid sibling cleanup child');
    const child = lstatIfPresent(path);
    if (!child) continue;
    if (!child.isFile() || child.isSymbolicLink()) {
      throw new Error(`refusing nonregular sibling cleanup child: ${name}`);
    }
    unlinkSync(path);
    if (lstatIfPresent(path)) throw new Error(`sibling cleanup left residue: ${name}`);
  }
  guardAttemptRoot(tempDir);
  rmdirSync(tempDir);
  if (lstatIfPresent(tempDir)) throw new Error('sibling cleanup left root residue');
}

async function bounded(promise, label, timeoutMs = 2000) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label} exceeded ${timeoutMs}ms`)), timeoutMs);
      })
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function completeBarriers(receipt) {
  return receipt?.barriers.close && receipt.barriers.stdoutEOF && receipt.barriers.stderrEOF;
}

async function publishObservedReceipt(tempDir, observed) {
  const finalPath = join(tempDir, 'receipt.json');
  const temporaryPath = join(tempDir, 'receipt.json.tmp');
  const expected = canonicalReceiptBytes(observed);
  const result = await bounded(
    writeAtomicTerminalReceipt({ directory: tempDir, bytes: expected }),
    'atomic receipt publication'
  );
  const actual = readFileSync(finalPath);
  const mode = lstatSync(finalPath).mode & 0o777;
  const canonical = actual.equals(canonicalReceiptBytes(JSON.parse(actual.toString('utf8'))));
  if (!actual.equals(expected) || !result.bytes.equals(expected) || !canonical || mode !== 0o600) {
    throw new Error('observed terminal receipt is not exact');
  }
  return {
    raw: actual.toString('utf8'),
    evidence: {
      exact: true,
      canonical,
      mode,
      finalPresentBeforeCleanup: Boolean(lstatIfPresent(finalPath)),
      temporaryAbsentBeforeCleanup: !lstatIfPresent(temporaryPath)
    }
  };
}

async function runCrossPrimitiveCase(scenario) {
  const tempDir = realpathSync(mkdtempSync(join(tmpdir(), 'probe-governed-siblings-')));
  const readyFile = join(tempDir, 'worker-ready');
  const decisionFile = join(tempDir, 'controller-decision');
  const controllerArgs = [
    join(fixtureDir, 'controller.mjs'),
    scenario,
    ...(scenario === 'deadline' ? [decisionFile] : [])
  ];
  const worker = spawnGovernedProcess({
    command: process.execPath,
    args: [join(fixtureDir, 'worker.mjs'), 'hung', readyFile],
    terminationGraceMs: 50,
    cleanupTimeoutMs: 1000
  });
  const controller = spawnGovernedProcess({
    command: process.execPath,
    args: controllerArgs,
    terminationGraceMs: 50,
    cleanupTimeoutMs: 1000
  });
  let channel;
  let channelReceipt;
  let channelState;
  let controllerReceipt;
  let workerReceipt;
  let cleanupComplete = false;
  const ledger = [];
  const observeLocal = event => ledger.push({ ordinal: ledger.length + 1, event });

  try {
    await waitForFile(readyFile);
    let primaryClassification;
    if (scenario === 'crash') {
      controllerReceipt = await bounded(controller.result, 'crash controller terminal');
      primaryClassification = controllerReceipt.exitCode === 0 ? 'PASS' : 'FAIL_CONTROLLER';
      workerReceipt = await bounded(worker.terminate('controller-crash'), 'crash worker terminal');
    } else if (scenario === 'callback-failure') {
      channel = createAcknowledgedJsonlChannel({
        onRecord: record => {
          if (record.id !== 1 || record.value !== scenario) throw new Error('invalid decision');
          throw new Error('injected callback failure');
        },
        idleTimeoutMs: 1000,
        deadlineMs: 2000
      });
      controllerReceipt = await bounded(controller.result, 'callback controller terminal');
      await channel.write(controllerReceipt.stdout);
      await channel.end();
      channelReceipt = await bounded(channel.result, 'callback channel terminal');
      await channel.cleanup();
      channelState = channel.snapshot();
      primaryClassification = channelReceipt.classification;
      workerReceipt = await bounded(worker.terminate('callback-failure'), 'callback worker terminal');
    } else if (scenario === 'deadline') {
      channel = createAcknowledgedJsonlChannel({
        onRecord: record => {
          if (record.id !== 1 || record.value !== scenario) throw new Error('invalid decision');
          return new Promise(() => {});
        },
        idleTimeoutMs: 0,
        deadlineMs: 60
      });
      await waitForFile(decisionFile);
      const decision = readFileSync(decisionFile);
      const expectedDecision = canonicalReceiptBytes({ id: 1, value: scenario });
      if (!decision.equals(expectedDecision)) throw new Error('controller decision file is not exact');
      await channel.write(decision);
      await channel.end();
      channelReceipt = await bounded(channel.result, 'deadline channel terminal', 1000);
      await channel.cleanup();
      channelState = channel.snapshot();
      primaryClassification = channelReceipt.classification;
      observeLocal('worker-termination-begin');
      workerReceipt = await bounded(worker.terminate('global-deadline'), 'deadline worker terminal');
      observeLocal('worker-terminal');
      observeLocal('controller-termination-begin');
      controllerReceipt = await bounded(controller.terminate('global-deadline'), 'deadline controller terminal');
      if (controllerReceipt.stdout !== decision.toString('utf8')) {
        throw new Error('controller stdout does not mirror its decision file');
      }
    } else {
      throw new Error('invalid cross-primitive scenario');
    }

    if (!completeBarriers(workerReceipt) || !completeBarriers(controllerReceipt)) {
      throw new Error('cross-primitive process barriers are incomplete');
    }
    if (channel && (channelState.pending !== 0 || channelState.writes !== 0 ||
        channelState.timers !== 0 || channelState.listeners !== 0)) {
      throw new Error('cross-primitive channel cleanup is incomplete');
    }
    const observed = {
      classification: primaryClassification,
      ...(channelReceipt ? { channel: channelReceipt, channelState } : {}),
      controller: aggregateReceipt(controllerReceipt),
      ...(ledger.length > 0 ? { ledger } : {}),
      worker: aggregateReceipt(workerReceipt)
    };
    const published = await publishObservedReceipt(tempDir, observed);
    cleanupExactAttempt(tempDir);
    cleanupComplete = true;
    return {
      ...observed,
      controllerArgs,
      controllerId: controller.id,
      controllerRaw: controllerReceipt,
      publishedRaw: published.raw,
      receiptEvidence: published.evidence,
      residue: {
        controllerDecision: lstatIfPresent(decisionFile) ? 1 : 0,
        controllerDecisionTemporary: lstatIfPresent(`${decisionFile}.tmp`) ? 1 : 0,
        directory: lstatIfPresent(tempDir) ? 1 : 0,
        final: lstatIfPresent(join(tempDir, 'receipt.json')) ? 1 : 0,
        temporary: lstatIfPresent(join(tempDir, 'receipt.json.tmp')) ? 1 : 0
      },
      workerId: worker.id,
      workerObserved: workerReceipt.observed
    };
  } finally {
    await Promise.allSettled([
      channel?.cleanup(),
      worker.terminate('cross-fixture-cleanup'),
      controller.terminate('cross-fixture-cleanup')
    ]);
    if (!cleanupComplete) cleanupExactAttempt(tempDir);
  }
}

function expectTermKillExitClose(aggregateReceiptValue, observed) {
  expect(aggregateReceiptValue).toMatchObject({
    classification: 'terminated',
    exitCode: null,
    signal: 'SIGKILL',
    barriers: { close: true, stdoutEOF: true, stderrEOF: true },
    signalAttempts: [
      { signal: 'SIGTERM', accepted: true },
      { signal: 'SIGKILL', accepted: true }
    ]
  });
  const term = observed.findIndex(event => event.fact === 'signal-attempt' && event.signal === 'SIGTERM');
  const kill = observed.findIndex(event => event.fact === 'signal-attempt' && event.signal === 'SIGKILL');
  const exit = observed.findIndex(event => event.fact === 'exit');
  const close = observed.findIndex(event => event.fact === 'barrier' && event.barrier === 'close');
  expect(term).toBeGreaterThanOrEqual(0);
  expect(term).toBeLessThan(kill);
  expect(kill).toBeLessThan(exit);
  expect(exit).toBeLessThan(close);
}

function expectExactCrossPublication(aggregate) {
  const expected = {
    classification: aggregate.classification,
    ...(aggregate.channel ? { channel: aggregate.channel, channelState: aggregate.channelState } : {}),
    controller: aggregate.controller,
    ...(aggregate.ledger ? { ledger: aggregate.ledger } : {}),
    worker: aggregate.worker
  };
  expect(JSON.parse(aggregate.publishedRaw)).toEqual(expected);
  expect(aggregate).toMatchObject({
    receiptEvidence: {
      exact: true,
      canonical: true,
      mode: 0o600,
      finalPresentBeforeCleanup: true,
      temporaryAbsentBeforeCleanup: true
    },
    residue: {
      controllerDecision: 0,
      controllerDecisionTemporary: 0,
      directory: 0,
      final: 0,
      temporary: 0
    }
  });
  expect(aggregate.controllerId).not.toBe(aggregate.workerId);
  expect(aggregate.controllerArgs.join('\0')).not.toContain(aggregate.controllerId);
  expect(aggregate.controllerArgs.join('\0')).not.toContain(aggregate.workerId);
  expect(aggregate.controllerRaw.stdout).not.toContain(aggregate.controllerId);
  expect(aggregate.controllerRaw.stdout).not.toContain(aggregate.workerId);
  expect(JSON.stringify(aggregate.controllerRaw)).not.toContain(aggregate.workerId);
  expect(JSON.stringify(aggregate.controllerRaw)).not.toContain('pid');
  expect(aggregate.publishedRaw).not.toContain(aggregate.controllerId);
  expect(aggregate.publishedRaw).not.toContain(aggregate.workerId);
  expect(aggregate.publishedRaw).not.toContain('pid');
}

async function runSiblingCase(mode, { publicationFailure = false, cleanupFailure = false } = {}) {
  const tempDir = realpathSync(mkdtempSync(join(tmpdir(), 'probe-governed-siblings-')));
  const readyFile = join(tempDir, 'worker-ready');
  const finalPath = join(tempDir, 'receipt.json');
  const temporaryPath = join(tempDir, 'receipt.json.tmp');
  let workerReceipt;
  let controllerReceipt;
  let primaryClassification;
  let classification;
  let finalizationPromise;
  let finalizationCalls = 0;
  let receiptWriterAttempts = 0;
  let cleanupAttempts = 0;
  let cleanupComplete = false;
  let cleanupFailureObserved = false;
  let receiptEvidence;

  const finalize = bytes => {
    finalizationCalls += 1;
    if (!finalizationPromise) {
      receiptWriterAttempts += 1;
      finalizationPromise = writeAtomicTerminalReceipt({ directory: tempDir, bytes });
    }
    return finalizationPromise;
  };
  const cleanup = injectFailure => {
    if (cleanupComplete) return;
    cleanupAttempts += 1;
    guardAttemptRoot(tempDir);
    if (injectFailure) {
      try { rmdirSync(tempDir); }
      catch (error) {
        cleanupFailureObserved = true;
        throw error;
      }
      throw new Error('nonempty cleanup unexpectedly succeeded');
    }
    cleanupExactAttempt(tempDir);
    cleanupComplete = true;
  };

  const worker = spawnGovernedProcess({
    command: process.execPath,
    args: [join(fixtureDir, 'worker.mjs'), mode, readyFile],
    terminationGraceMs: 50,
    cleanupTimeoutMs: 1000
  });
  const controller = spawnGovernedProcess({
    command: process.execPath,
    args: [join(fixtureDir, 'controller.mjs'), mode],
    terminationGraceMs: 50,
    cleanupTimeoutMs: 1000
  });
  const channel = createAcknowledgedJsonlChannel({
    onRecord: async record => {
      if (record.id !== 1 || record.value !== mode) throw new Error('invalid decision');
      workerReceipt = mode === 'hung'
        ? await worker.terminate('controller-decision')
        : await worker.result;
      if (!workerReceipt.barriers.close || !workerReceipt.barriers.stdoutEOF || !workerReceipt.barriers.stderrEOF) {
        throw new Error('worker terminal barrier incomplete');
      }
    },
    idleTimeoutMs: 2000,
    deadlineMs: 4000
  });

  try {
    await waitForFile(readyFile);
    controllerReceipt = await controller.result;
    await channel.write(controllerReceipt.stdout);
    await channel.end();
    const channelReceipt = await channel.result;
    await channel.cleanup();
    primaryClassification = channelReceipt.classification;
    classification = primaryClassification;
    const channelState = channel.snapshot();
    if (primaryClassification !== 'PASS' || !completeBarriers(workerReceipt) ||
        !completeBarriers(controllerReceipt) || channelState.pending !== 0 ||
        channelState.writes !== 0 || channelState.timers !== 0 || channelState.listeners !== 0) {
      throw new Error('terminal publication barriers are incomplete');
    }
    const observed = {
      channel: channelReceipt,
      channelState,
      controller: aggregateReceipt(controllerReceipt),
      worker: aggregateReceipt(workerReceipt)
    };
    const receiptBytes = canonicalReceiptBytes(observed);
    if (publicationFailure) writeFileSync(finalPath, 'occupied-final');
    let publicationError;
    try {
      const firstFinalization = finalize(receiptBytes);
      const repeatedFinalization = finalize(receiptBytes);
      if (firstFinalization !== repeatedFinalization) throw new Error('finalization was not idempotent');
      const result = await firstFinalization;
      const actual = readFileSync(finalPath);
      const mode = lstatSync(finalPath).mode & 0o777;
      const canonical = actual.equals(canonicalReceiptBytes(JSON.parse(actual.toString('utf8'))));
      if (!actual.equals(receiptBytes) || !result.bytes.equals(receiptBytes) || !canonical || mode !== 0o600) {
        throw new Error('observed terminal receipt is not exact');
      }
      receiptEvidence = {
        exact: true,
        canonical,
        mode,
        size: actual.length,
        finalPresentBeforeCleanup: existsSync(finalPath),
        temporaryAbsentBeforeCleanup: !existsSync(temporaryPath)
      };
    } catch (error) {
      publicationError = error;
      try { await finalize(receiptBytes); }
      catch (repeatedError) {
        if (repeatedError !== error) throw new Error('failed finalization did not retain its first result');
      }
      classification = 'FAIL_CLEANUP_OR_IMMUTABILITY';
      receiptEvidence = {
        exact: false,
        finalPresentBeforeCleanup: existsSync(finalPath),
        temporaryAbsentBeforeCleanup: !existsSync(temporaryPath)
      };
    }

    if (cleanupFailure) {
      try { cleanup(true); }
      catch {
        classification = 'FAIL_CLEANUP_OR_IMMUTABILITY';
      }
    }
    cleanup(false);
    const completedCleanupAttempts = cleanupAttempts;
    cleanup(false);

    return {
      ...observed,
      classification,
      primaryClassification,
      publicationError: publicationError?.message ?? null,
      receiptEvidence,
      receiptWriterAttempts,
      finalizationCalls,
      cleanupAttempts,
      cleanupFailureObserved,
      cleanupIdempotent: cleanupAttempts === completedCleanupAttempts,
      residue: {
        directory: existsSync(tempDir) ? 1 : 0,
        final: existsSync(finalPath) ? 1 : 0,
        temporary: existsSync(temporaryPath) ? 1 : 0
      },
      controllerRaw: controllerReceipt,
      workerObserved: workerReceipt.observed,
      controllerId: controller.id,
      workerId: worker.id
    };
  } finally {
    await Promise.allSettled([
      channel.cleanup(),
      worker.terminate('fixture-cleanup'),
      controller.terminate('fixture-cleanup')
    ]);
    if (!cleanupComplete) cleanup(false);
  }
}

describe('governed sibling recombination', () => {
  test('normal worker reaches channel PASS without signals', async () => {
    const aggregate = await runSiblingCase('normal');
    expect(aggregate).toMatchObject({
      classification: 'PASS',
      primaryClassification: 'PASS',
      receiptWriterAttempts: 1,
      finalizationCalls: 2,
      cleanupAttempts: 1,
      cleanupIdempotent: true,
      cleanupFailureObserved: false,
      residue: { directory: 0, final: 0, temporary: 0 },
      receiptEvidence: {
        exact: true,
        canonical: true,
        mode: 0o600,
        finalPresentBeforeCleanup: true,
        temporaryAbsentBeforeCleanup: true
      }
    });
    expect(aggregate.channel).toMatchObject({ classification: 'PASS', frames: 1, acknowledgements: 1, eof: true });
    expect(aggregate.channelState).toMatchObject({ pending: 0, writes: 0, timers: 0, listeners: 0, cleaned: true });
    expect(aggregate.worker).toEqual({
      classification: 'exited',
      exitCode: 0,
      signal: null,
      barriers: { close: true, stdoutEOF: true, stderrEOF: true },
      signalAttempts: []
    });
    expect(aggregate.controller).toEqual({
      classification: 'exited',
      exitCode: 0,
      signal: null,
      barriers: { close: true, stdoutEOF: true, stderrEOF: true },
      signalAttempts: []
    });
    expect(aggregate.controllerId).not.toBe(aggregate.workerId);
  });

  test('hung worker is killed and fully closed before its ACK permits PASS', async () => {
    const aggregate = await runSiblingCase('hung');
    expect(aggregate).toMatchObject({
      classification: 'PASS',
      primaryClassification: 'PASS',
      receiptWriterAttempts: 1,
      cleanupIdempotent: true,
      residue: { directory: 0, final: 0, temporary: 0 },
      receiptEvidence: { exact: true, canonical: true, mode: 0o600, temporaryAbsentBeforeCleanup: true }
    });
    expect(aggregate.channel).toMatchObject({ classification: 'PASS', frames: 1, acknowledgements: 1, eof: true });
    expect(aggregate.worker).toEqual({
      classification: 'terminated',
      exitCode: null,
      signal: 'SIGKILL',
      barriers: { close: true, stdoutEOF: true, stderrEOF: true },
      signalAttempts: [
        { signal: 'SIGTERM', accepted: true },
        { signal: 'SIGKILL', accepted: true }
      ]
    });
    expect(aggregate.controller).toEqual({
      classification: 'exited',
      exitCode: 0,
      signal: null,
      barriers: { close: true, stdoutEOF: true, stderrEOF: true },
      signalAttempts: []
    });
    expect(aggregate.channelState).toMatchObject({ pending: 0, writes: 0, timers: 0, listeners: 0, cleaned: true });
    const termIndex = aggregate.workerObserved.findIndex(event => event.fact === 'signal-attempt' && event.signal === 'SIGTERM');
    const killIndex = aggregate.workerObserved.findIndex(event => event.fact === 'signal-attempt' && event.signal === 'SIGKILL');
    const exitIndex = aggregate.workerObserved.findIndex(event => event.fact === 'exit');
    const closeIndex = aggregate.workerObserved.findIndex(event => event.fact === 'barrier' && event.barrier === 'close');
    expect(termIndex).toBeGreaterThanOrEqual(0);
    expect(termIndex).toBeLessThan(killIndex);
    expect(killIndex).toBeLessThan(exitIndex);
    expect(exitIndex).toBeLessThan(closeIndex);
    expect(aggregate.controllerId).not.toBe(aggregate.workerId);
    expect(aggregate.controllerRaw.stdout).not.toContain(aggregate.controllerId);
    expect(aggregate.controllerRaw.stdout).not.toContain(aggregate.workerId);
    expect(JSON.stringify(aggregate.controllerRaw)).not.toContain('pid');
    expect(JSON.parse(aggregate.controllerRaw.stdout)).toEqual({ id: 1, value: 'hung' });
  });

  test('receipt publication failure dominates final classification but retains primary PASS', async () => {
    const aggregate = await runSiblingCase('normal', { publicationFailure: true });
    expect(aggregate).toMatchObject({
      classification: 'FAIL_CLEANUP_OR_IMMUTABILITY',
      primaryClassification: 'PASS',
      receiptWriterAttempts: 1,
      finalizationCalls: 3,
      cleanupIdempotent: true,
      residue: { directory: 0, final: 0, temporary: 0 },
      receiptEvidence: { exact: false, temporaryAbsentBeforeCleanup: true }
    });
    expect(aggregate.publicationError).toContain('occupied path');
  });

  test('real cleanup failure retries the same cleanup and retains dominance with zero residue', async () => {
    const aggregate = await runSiblingCase('normal', { cleanupFailure: true });
    expect(aggregate).toMatchObject({
      classification: 'FAIL_CLEANUP_OR_IMMUTABILITY',
      primaryClassification: 'PASS',
      receiptWriterAttempts: 1,
      finalizationCalls: 2,
      cleanupAttempts: 2,
      cleanupFailureObserved: true,
      cleanupIdempotent: true,
      residue: { directory: 0, final: 0, temporary: 0 },
      receiptEvidence: {
        exact: true,
        canonical: true,
        finalPresentBeforeCleanup: true,
        temporaryAbsentBeforeCleanup: true
      }
    });
  });

  test('controller crash fully terminates before the opaque TERM-ignoring worker is reaped', async () => {
    const aggregate = await runCrossPrimitiveCase('crash');
    expect(aggregate.classification).toBe('FAIL_CONTROLLER');
    expect(aggregate.controller).toEqual({
      classification: 'exited',
      exitCode: 17,
      signal: null,
      barriers: { close: true, stdoutEOF: true, stderrEOF: true },
      signalAttempts: []
    });
    expect(aggregate.controllerRaw).toMatchObject({
      stdout: '',
      stdoutBytes: 0,
      stderr: '',
      stderrBytes: 0
    });
    expectTermKillExitClose(aggregate.worker, aggregate.workerObserved);
    expectExactCrossPublication(aggregate);
  }, 5000);

  test('callback failure fails the channel then explicitly reaps the worker before publication', async () => {
    const aggregate = await runCrossPrimitiveCase('callback-failure');
    expect(aggregate.classification).toBe('FAIL_ACK');
    expect(aggregate.channel).toMatchObject({
      classification: 'FAIL_ACK',
      error: 'injected callback failure',
      frames: 1,
      acknowledgements: 0,
      eof: false
    });
    expect(aggregate.channelState).toMatchObject({
      firstFailureCount: 1,
      pending: 0,
      writes: 0,
      timers: 0,
      listeners: 0,
      cleaned: true
    });
    expect(aggregate.controller).toEqual({
      classification: 'exited',
      exitCode: 0,
      signal: null,
      barriers: { close: true, stdoutEOF: true, stderrEOF: true },
      signalAttempts: []
    });
    expectTermKillExitClose(aggregate.worker, aggregate.workerObserved);
    expect(JSON.parse(aggregate.controllerRaw.stdout)).toEqual({ id: 1, value: 'callback-failure' });
    expectExactCrossPublication(aggregate);
  }, 5000);

  test('global deadline fully reaps worker before terminating the TERM-ignoring controller', async () => {
    const aggregate = await runCrossPrimitiveCase('deadline');
    expect(aggregate.classification).toBe('FAIL_DEADLINE');
    expect(aggregate.channel).toMatchObject({
      classification: 'FAIL_DEADLINE',
      frames: 1,
      acknowledgements: 0,
      eof: true
    });
    expect(aggregate.channelState).toMatchObject({
      firstFailureCount: 1,
      abortCount: 1,
      pending: 0,
      writes: 0,
      timers: 0,
      listeners: 0,
      cleaned: true
    });
    expect(aggregate.ledger).toEqual([
      { ordinal: 1, event: 'worker-termination-begin' },
      { ordinal: 2, event: 'worker-terminal' },
      { ordinal: 3, event: 'controller-termination-begin' }
    ]);
    expectTermKillExitClose(aggregate.worker, aggregate.workerObserved);
    expectTermKillExitClose(aggregate.controller, aggregate.controllerRaw.observed);
    expect(JSON.parse(aggregate.controllerRaw.stdout)).toEqual({ id: 1, value: 'deadline' });
    expectExactCrossPublication(aggregate);
  }, 5000);
});
