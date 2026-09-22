import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as governance from '@probelabs/probe/agent/governance';
import { spawnGovernedProcess as directSpawn } from '../../src/agent/processSupervisor.js';
import { createAcknowledgedJsonlChannel as directChannel } from '../../src/agent/governance/acknowledgedJsonlChannel.js';
import { writeAtomicTerminalReceipt as directReceipt } from '../../src/agent/governance/atomicTerminalReceipt.js';

const packageRoot = fileURLToPath(new URL('../..', import.meta.url));

function linkedConsumer() {
  const directory = mkdtempSync(join(tmpdir(), 'probe-governance-consumer-'));
  const packageLink = join(directory, 'node_modules', '@probelabs', 'probe');
  mkdirSync(dirname(packageLink), { recursive: true });
  symlinkSync(packageRoot, packageLink, 'dir');
  writeFileSync(join(directory, 'package.json'), JSON.stringify({ type: 'module' }));
  return directory;
}

test('governance subpath exposes exactly the accepted implementations', () => {
  assert.deepEqual(Object.keys(governance).sort(), [
    'createAcknowledgedJsonlChannel',
    'spawnGovernedProcess',
    'writeAtomicTerminalReceipt'
  ]);
  assert.equal(governance.spawnGovernedProcess, directSpawn);
  assert.equal(governance.createAcknowledgedJsonlChannel, directChannel);
  assert.equal(governance.writeAtomicTerminalReceipt, directReceipt);
});

test('governance subpath does not load the package root or dotenv', () => {
  const directory = linkedConsumer();
  try {
    writeFileSync(join(directory, '.env'), 'PROBE_GOVERNANCE_DOTENV_SENTINEL=loaded\n');
    const result = spawnSync(process.execPath, [
      '--input-type=module',
      '--eval',
      "await import('@probelabs/probe/agent/governance'); process.stdout.write(process.env.PROBE_GOVERNANCE_DOTENV_SENTINEL ?? 'absent')"
    ], { cwd: directory, encoding: 'utf8', env: { ...process.env, PROBE_GOVERNANCE_DOTENV_SENTINEL: undefined } });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, 'absent');
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('governance declarations support NodeNext consumers and reject facade additions', () => {
  const directory = linkedConsumer();
  try {
    writeFileSync(join(directory, 'consumer.ts'), `
      import {
        spawnGovernedProcess, createAcknowledgedJsonlChannel, writeAtomicTerminalReceipt,
        type GovernedProcessReceipt, type AcknowledgedJsonlRecord,
        type AtomicTerminalReceiptResult
      } from '@probelabs/probe/agent/governance';
      const processHandle = spawnGovernedProcess({ command: process.execPath, args: ['--version'], signalScope: 'child' });
      const processResult: Promise<GovernedProcessReceipt> = processHandle.result;
      const channel = createAcknowledgedJsonlChannel({ onRecord(record: AcknowledgedJsonlRecord, signal) { void record.value; void signal.aborted; } });
      const acceptedWrite: Promise<boolean> = channel.write('{"id":1,"value":"ok"}\\n');
      const receipt: Promise<Readonly<AtomicTerminalReceiptResult>> = writeAtomicTerminalReceipt({ directory: '/tmp', bytes: '{}' });
      void processResult; void acceptedWrite; void receipt;
      // @ts-expect-error the subpath has no lifecycle facade
      import('@probelabs/probe/agent/governance').then(module => module.ProbeAgent);
    `);
    writeFileSync(join(directory, 'tsconfig.json'), JSON.stringify({
      compilerOptions: {
        target: 'ES2022', module: 'NodeNext', moduleResolution: 'NodeNext', strict: true,
        noEmit: true, skipLibCheck: true
      },
      files: ['consumer.ts']
    }));
    const result = spawnSync(process.execPath, [
      join(packageRoot, 'node_modules', 'typescript', 'bin', 'tsc'),
      '-p', join(directory, 'tsconfig.json')
    ], { encoding: 'utf8' });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('public primitives retain safe invocation and validation behavior', async () => {
  assert.throws(() => governance.spawnGovernedProcess({ command: '' }), /non-empty string/);
  assert.throws(() => governance.createAcknowledgedJsonlChannel(), /onRecord must be a function/);
  await assert.rejects(governance.writeAtomicTerminalReceipt(), /bytes must be/);

  const governed = governance.spawnGovernedProcess({
    command: process.execPath,
    args: ['--eval', "process.stdout.write('governed')"],
    terminationGraceMs: 100,
    cleanupTimeoutMs: 2000
  });
  assert.equal((await governed.result).stdout, 'governed');

  const channel = governance.createAcknowledgedJsonlChannel({
    onRecord: () => undefined,
    idleTimeoutMs: 1000,
    deadlineMs: 2000
  });
  await channel.write('{"id":1,"value":"acknowledged"}\n');
  await channel.end();
  assert.equal((await channel.result).classification, 'PASS');
  await channel.cleanup();

  const directory = realpathSync(mkdtempSync(join(tmpdir(), 'probe-governance-receipt-')));
  try {
    const receipt = await governance.writeAtomicTerminalReceipt({ directory, bytes: '{"ok":true}' });
    assert.equal(receipt.mode, 0o600);
    assert.equal(readFileSync(join(directory, 'receipt.json'), 'utf8'), '{"ok":true}');
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
