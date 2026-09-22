import { describe, expect, jest, test } from '@jest/globals';
import { existsSync, mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { spawnGovernedProcess } from '../../src/agent/processSupervisor.js';

const nodeProcess = (source, options = {}) => spawnGovernedProcess({
  command: process.execPath,
  args: ['-e', source],
  cleanupTimeoutMs: 2000,
  terminationGraceMs: 100,
  ...options
});

async function waitForFile(file, timeoutMs = 2000) {
  const deadline = Date.now() + timeoutMs;
  while (!existsSync(file)) {
    if (Date.now() >= deadline) throw new Error(`Fixture did not become ready: ${file}`);
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

describe('spawnGovernedProcess', () => {
  test('settles normal execution only after close and both stream EOF barriers', async () => {
    const governed = nodeProcess(`
      process.stdout.write('stdout-value');
      process.stderr.write('stderr-value');
    `);

    expect(Object.keys(governed).sort()).toEqual(['id', 'result', 'terminate']);
    expect(governed).not.toHaveProperty('pid');

    const receipt = await governed.result;
    expect(receipt.classification).toBe('exited');
    expect(receipt.exitCode).toBe(0);
    expect(receipt.stdout).toBe('stdout-value');
    expect(receipt.stderr).toBe('stderr-value');
    expect(receipt.barriers).toEqual({ close: true, stdoutEOF: true, stderrEOF: true });
    expect(receipt.observed.map(event => event.sequence)).toEqual(
      receipt.observed.map((_, index) => index + 1)
    );
    expect(receipt.observed).toEqual(expect.arrayContaining([
      expect.objectContaining({ fact: 'exit', code: 0 }),
      expect.objectContaining({ fact: 'barrier', barrier: 'close' }),
      expect.objectContaining({ fact: 'barrier', barrier: 'stdoutEOF' }),
      expect.objectContaining({ fact: 'barrier', barrier: 'stderrEOF' })
    ]));
    expect(JSON.stringify(receipt)).not.toContain('pid');
  });

  test('records exit before delayed descendant stdio EOF without stale termination', async () => {
    const startedAt = Date.now();
    const governed = nodeProcess(`
      const { spawn } = require('child_process');
      const descendant = spawn(process.execPath, ['-e', "setTimeout(() => { process.stdout.write('descendant-output'); process.stderr.write('descendant-error'); }, 600)"], {
        stdio: ['ignore', 1, 2]
      });
      descendant.unref();
    `, { executionTimeoutMs: 200 });

    const receipt = await governed.result;
    const elapsedMs = Date.now() - startedAt;
    const exitFact = receipt.observed.find(event => event.fact === 'exit');
    const barrierFacts = receipt.observed.filter(event => event.fact === 'barrier');

    expect(receipt.classification).toBe('exited');
    expect(receipt.exitCode).toBe(0);
    expect(receipt.stdout).toBe('descendant-output');
    expect(receipt.stderr).toBe('descendant-error');
    expect(elapsedMs).toBeGreaterThanOrEqual(500);
    expect(receipt.barriers).toEqual({ close: true, stdoutEOF: true, stderrEOF: true });
    expect(receipt.observed.filter(event => event.fact === 'signal-attempt')).toEqual([]);
    expect(barrierFacts.every(event => event.sequence > exitFact.sequence)).toBe(true);
  });

  test('escalates a TERM-ignoring process to KILL on the fixed grace deadline', async () => {
    const fixtureDir = mkdtempSync(join(tmpdir(), 'probe-supervisor-ready-'));
    const readyFile = join(fixtureDir, 'ready');
    try {
      const governed = spawnGovernedProcess({
        command: process.execPath,
        args: ['-e', `
          process.on('SIGTERM', () => {});
          require('fs').writeFileSync(process.argv[1], 'READY');
          setInterval(() => {}, 1000);
        `, readyFile],
        terminationGraceMs: 80,
        cleanupTimeoutMs: 2000
      });

      await waitForFile(readyFile);
      const receipt = await governed.terminate('fixture-ready');
      const attempts = receipt.observed.filter(event => event.fact === 'signal-attempt');
      expect(receipt.classification).toBe('terminated');
      expect(receipt.signal).toBe('SIGKILL');
      expect(attempts.map(event => event.signal)).toEqual(['SIGTERM', 'SIGKILL']);
      expect(attempts).toEqual(expect.arrayContaining([
        expect.objectContaining({ requestedScope: 'child', actualScope: 'child' })
      ]));
      expect(receipt.observed).toEqual(expect.arrayContaining([
        expect.objectContaining({
          fact: 'signal',
          signal: 'SIGKILL',
          requestedScope: 'child',
          actualScope: 'child'
        })
      ]));
    } finally {
      rmSync(fixtureDir, { recursive: true, force: true });
    }
  });

  test('records child scope when process-group delivery falls back to the child', async () => {
    const fixtureDir = mkdtempSync(join(tmpdir(), 'probe-supervisor-fallback-'));
    const readyFile = join(fixtureDir, 'ready');
    const originalProcessKill = process.kill;
    let killSpy;
    try {
      const governed = spawnGovernedProcess({
        command: process.execPath,
        args: ['-e', `
          require('fs').writeFileSync(process.argv[1], 'READY');
          setInterval(() => {}, 1000);
        `, readyFile],
        signalScope: 'process-group',
        terminationGraceMs: 100,
        cleanupTimeoutMs: 2000
      });

      await waitForFile(readyFile);
      killSpy = jest.spyOn(process, 'kill').mockImplementation((pid, signal) => {
        if (pid < 0) throw new Error('forced process-group delivery failure');
        return originalProcessKill.call(process, pid, signal);
      });
      const result = governed.terminate('fallback-fixture');
      killSpy.mockRestore();
      killSpy = null;

      const receipt = await result;
      expect(receipt.classification).toBe('terminated');
      expect(receipt.observed).toEqual(expect.arrayContaining([
        expect.objectContaining({
          fact: 'signal-attempt',
          signal: 'SIGTERM',
          requestedScope: 'process-group',
          actualScope: 'child',
          accepted: true
        }),
        expect.objectContaining({
          fact: 'signal',
          signal: 'SIGTERM',
          requestedScope: 'process-group',
          actualScope: 'child'
        })
      ]));
    } finally {
      killSpy?.mockRestore();
      rmSync(fixtureDir, { recursive: true, force: true });
    }
  });

  test('bounds each drain and terminates on output overflow', async () => {
    const governed = nodeProcess(`
      process.stdout.write(Buffer.alloc(4096, 'x'));
      setInterval(() => {}, 1000);
    `, {
      stdoutByteCap: 128,
      stderrByteCap: 64,
      terminationGraceMs: 20
    });

    const receipt = await governed.result;
    expect(receipt.classification).toBe('output_overflow');
    expect(receipt.reason).toBe('stdout_overflow');
    expect(receipt.stdoutBytes).toBe(128);
    expect(Buffer.byteLength(receipt.stdout)).toBe(128);
    expect(receipt.stderrBytes).toBeLessThanOrEqual(64);
  });

  test('settles a spawn error exactly once', async () => {
    const governed = spawnGovernedProcess({
      command: `missing-governed-process-command-${Date.now()}`
    });
    let settlements = 0;
    governed.result.then(() => { settlements += 1; });

    const [receipt, terminatedReceipt] = await Promise.all([
      governed.result,
      governed.terminate('too-late')
    ]);
    await Promise.resolve();

    expect(receipt).toBe(terminatedReceipt);
    expect(receipt.classification).toBe('spawn_error');
    expect(receipt.error).toBeTruthy();
    expect(receipt.observed).toEqual(expect.arrayContaining([
      expect.objectContaining({ fact: 'spawn-error' })
    ]));
    expect(settlements).toBe(1);
  });

  test('settles an aborted process exactly once despite later termination requests', async () => {
    const controller = new AbortController();
    const governed = nodeProcess('setInterval(() => {}, 1000)', {
      signal: controller.signal,
      terminationGraceMs: 20
    });
    let settlements = 0;
    governed.result.then(() => { settlements += 1; });

    controller.abort();
    const [receipt, terminatedReceipt] = await Promise.all([
      governed.result,
      governed.terminate('duplicate-request')
    ]);
    await Promise.resolve();

    expect(receipt).toBe(terminatedReceipt);
    expect(receipt.classification).toBe('aborted');
    expect(receipt.reason).toBe('aborted');
    expect(settlements).toBe(1);
  });

  test('rejects an invalid AbortSignal before spawning a child', async () => {
    const fixtureDir = mkdtempSync(join(tmpdir(), 'probe-supervisor-invalid-signal-'));
    const spawnedFile = join(fixtureDir, 'spawned');
    try {
      expect(() => spawnGovernedProcess({
        command: process.execPath,
        args: ['-e', "require('fs').writeFileSync(process.argv[1], 'spawned')", spawnedFile],
        signal: { aborted: false }
      })).toThrow('signal must be an AbortSignal');
      await new Promise(resolve => setTimeout(resolve, 30));
      expect(existsSync(spawnedFile)).toBe(false);
    } finally {
      rmSync(fixtureDir, { recursive: true, force: true });
    }
  });

  test('classifies cleanup timeout and performs no signals or receipt work after result', async () => {
    const fixtureDir = mkdtempSync(join(tmpdir(), 'probe-supervisor-cleanup-'));
    const readyFile = join(fixtureDir, 'ready');
    try {
      const governed = spawnGovernedProcess({
        command: process.execPath,
        args: ['-e', `
          const { spawn } = require('child_process');
          process.on('SIGTERM', () => {});
          spawn(process.execPath, ['-e', 'setTimeout(() => {}, 250)'], { stdio: ['ignore', 1, 2] });
          require('fs').writeFileSync(process.argv[1], 'READY');
          setInterval(() => {}, 1000);
        `, readyFile],
        signalScope: 'child',
        terminationGraceMs: 20,
        cleanupTimeoutMs: 60
      });

      await waitForFile(readyFile);
      const receipt = await governed.terminate('cleanup-fixture');
      expect(receipt.classification).toBe('cleanup_timeout');
      expect(receipt.observed.filter(event => event.fact === 'signal-attempt').map(event => event.signal))
        .toEqual(['SIGTERM', 'SIGKILL']);
      const observedCount = receipt.observed.length;

      const repeatedReceipt = await governed.terminate('after-result');
      await new Promise(resolve => setTimeout(resolve, 40));
      expect(repeatedReceipt).toBe(receipt);
      expect(receipt.observed).toHaveLength(observedCount);
    } finally {
      rmSync(fixtureDir, { recursive: true, force: true });
    }
  });
});
