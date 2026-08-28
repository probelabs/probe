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

// Test-only two-case recombination: no live protocol, seven-case runner,
// public sibling API, coordinator, fanout, Proof, Visor, Luna, model, provider,
// network, or API behavior is introduced here. Receipt publication is observed
// process atomicity, not power-loss directory-entry durability.
const fixtureDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'fixtures', 'governed-siblings');

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
    const root = lstatSync(tempDir);
    if (!root.isDirectory() || root.isSymbolicLink() || realpathSync(tempDir) !== tempDir) {
      throw new Error('sibling cleanup root lost its canonical nonsymlink boundary');
    }
    if (injectFailure) {
      try { rmdirSync(tempDir); }
      catch (error) {
        cleanupFailureObserved = true;
        throw error;
      }
      throw new Error('nonempty cleanup unexpectedly succeeded');
    }
    for (const name of ['worker-ready', 'receipt.json.tmp', 'receipt.json']) {
      const path = join(tempDir, name);
      if (dirname(path) !== tempDir || basename(path) !== name) throw new Error('invalid sibling cleanup child');
      const child = lstatIfPresent(path);
      if (!child) continue;
      if (!child.isFile() || child.isSymbolicLink()) {
        throw new Error(`refusing nonregular sibling cleanup child: ${name}`);
      }
      unlinkSync(path);
      if (lstatIfPresent(path)) throw new Error(`sibling cleanup left residue: ${name}`);
      const currentRoot = lstatSync(tempDir);
      if (!currentRoot.isDirectory() || currentRoot.isSymbolicLink() || realpathSync(tempDir) !== tempDir) {
        throw new Error('sibling cleanup root changed during cleanup');
      }
    }
    const finalRoot = lstatSync(tempDir);
    if (!finalRoot.isDirectory() || finalRoot.isSymbolicLink() || realpathSync(tempDir) !== tempDir) {
      throw new Error('sibling cleanup root changed before rmdir');
    }
    rmdirSync(tempDir);
    if (existsSync(tempDir)) throw new Error('sibling cleanup left root residue');
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
    const completeBarriers = receipt => receipt?.barriers.close &&
      receipt.barriers.stdoutEOF && receipt.barriers.stderrEOF;
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
});
