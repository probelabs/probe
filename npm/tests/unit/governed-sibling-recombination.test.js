import { describe, expect, test } from '@jest/globals';
import { existsSync, mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { createAcknowledgedJsonlChannel } from '../../src/agent/governance/acknowledgedJsonlChannel.js';
import { spawnGovernedProcess } from '../../src/agent/processSupervisor.js';

// Test-only two-case recombination: no live protocol, seven-case runner,
// finalizer, public sibling API, persistence, fanout, Proof, Visor, Luna, model,
// provider, network, or API behavior is introduced here.
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

async function runSiblingCase(mode) {
  const tempDir = mkdtempSync(join(tmpdir(), 'probe-governed-siblings-'));
  const readyFile = join(tempDir, 'worker-ready');
  let workerReceipt;
  let controllerReceipt;
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
    return {
      channel: channelReceipt,
      channelState: channel.snapshot(),
      controller: aggregateReceipt(controllerReceipt),
      worker: aggregateReceipt(workerReceipt),
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
    rmSync(tempDir, { recursive: true, force: true });
  }
}

describe('governed sibling recombination', () => {
  test('normal worker reaches channel PASS without signals', async () => {
    const aggregate = await runSiblingCase('normal');
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
});
