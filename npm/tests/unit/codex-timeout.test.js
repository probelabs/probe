import test from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { createCodexEngine } from '../../src/agent/engines/codex.js';

const fakeCodex = `#!/usr/bin/env node
import { createInterface } from 'node:readline';
import { writeFileSync } from 'node:fs';

const stateFile = process.env.CODEX_TIMEOUT_STATE;
const state = { pid: process.pid, requests: [] };
const save = () => writeFileSync(stateFile, JSON.stringify(state));
const send = value => process.stdout.write(JSON.stringify(value) + '\\n');
save();
process.on('SIGTERM', () => process.exit(0));
createInterface({ input: process.stdin }).on('line', line => {
  const request = JSON.parse(line);
  state.requests.push(request);
  save();
  if (request.method === 'initialize') {
    send({ jsonrpc: '2.0', id: request.id, result: {} });
    return;
  }
  if (request.method === 'tools/call' && !request.params.arguments.prompt.includes('[WAIT]')) {
    send({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text: 'ok' }] } });
  }
});
`;

async function withFakeCodex(fn) {
  const root = await mkdtemp(join(tmpdir(), 'probe-codex-timeout-'));
  const bin = join(root, 'bin');
  const stateFile = join(root, 'state.json');
  await mkdir(bin);
  await writeFile(join(bin, 'codex'), fakeCodex);
  await chmod(join(bin, 'codex'), 0o755);
  const originalPath = process.env.PATH;
  const originalState = process.env.CODEX_TIMEOUT_STATE;
  process.env.PATH = `${bin}:${originalPath}`;
  process.env.CODEX_TIMEOUT_STATE = stateFile;
  try {
    return await fn({ root, stateFile });
  } finally {
    process.env.PATH = originalPath;
    if (originalState === undefined) delete process.env.CODEX_TIMEOUT_STATE;
    else process.env.CODEX_TIMEOUT_STATE = originalState;
    await rm(root, { recursive: true, force: true });
  }
}

async function readState(stateFile) {
  return JSON.parse(await readFile(stateFile, 'utf8'));
}

async function collect(iterator) {
  const chunks = [];
  for await (const chunk of iterator) chunks.push(chunk);
  return chunks;
}

function isolatedWriterProfile(cwd) {
  return {
    version: 'probe.governed-codex-profile/v3', profileId: 'luna-xhigh-isolated-writer-v1', engine: 'codex',
    model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'workspace-write', approvalPolicy: 'never', cwd,
    probeMcpTools: ['search', 'extract', 'listFiles'], codexNativeTools: ['apply_patch', 'exec'],
    fallback: false, retries: 0,
  };
}

function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

test('Codex request timeout is bounded, diagnostic, and cleans up a hung child', async () => {
  await withFakeCodex(async ({ stateFile }) => {
    let engine;
    try {
      engine = await createCodexEngine({ requestTimeout: 1000 });
      const output = await collect(engine.query('[WAIT]'));
      const error = output.find(chunk => chunk.type === 'error')?.error;
      assert.equal(error?.message, 'Request tools/call timed out after 1000ms');
      const state = await readState(stateFile);
      assert.equal(state.requests.at(-1).method, 'tools/call');
      assert.equal(alive(state.pid), false);
    } finally {
      await engine?.close().catch(() => {});
    }
  });
});

test('governed Codex timeout emits one bounded record before normalized failure', async () => {
  await withFakeCodex(async ({ root, stateFile }) => {
    const cwd = realpathSync(root);
    const profile = isolatedWriterProfile(cwd);
    const agent = new ProbeAgent({ provider: 'codex', path: cwd, cwd, allowedTools: ['search', 'extract', 'listFiles'],
      governedCodexProfile: profile, requestTimeout: 1000, disableSkills: true });
    const timeouts = [];
    agent.events.on('timeout.request', record => timeouts.push(record));
    let engine;
    try {
      engine = await agent.getEngine();
      const output = await collect(engine.query('[WAIT]'));
      const error = output.find(chunk => chunk.type === 'error')?.error;
      assert.equal(error?.name, 'GovernedAnswerFailure');
      assert.equal(error?.answerFailureStage, 'provider_engine');
      assert.equal(error?.providerEngineFailureBoundary, 'query');
      assert.equal(error?.message, '');
      assert.deepEqual(timeouts, [{
        category: 'request_timeout', method: 'tools/call', boundary: 'query', timeout_ms: 1000,
        profileId: profile.profileId, sessionId: engine.sessionId,
      }]);
      assert.equal(Object.isFrozen(timeouts[0]), true);
      const serialized = JSON.stringify(timeouts[0]);
      for (const forbidden of [cwd, 'WAIT', 'prompt', 'params', 'environment', 'error'])
        assert.equal(serialized.includes(forbidden), false, forbidden);
      const state = await readState(stateFile);
      assert.equal(alive(state.pid), false);
    } finally {
      await agent.cleanup().catch(() => {});
    }
  });
});

test('ProbeAgent forwards a valid REQUEST_TIMEOUT value to Codex and restores the environment', async () => {
  await withFakeCodex(async ({ root, stateFile }) => {
    const originalRequestTimeout = process.env.REQUEST_TIMEOUT;
    process.env.REQUEST_TIMEOUT = '1000';
    let engine;
    try {
      const agent = new ProbeAgent({
        provider: 'codex',
        path: root,
        cwd: root,
        allowedTools: [],
        disableSkills: true
      });
      engine = await agent.getEngine();
      const output = await collect(engine.query('[WAIT]'));
      const error = output.find(chunk => chunk.type === 'error')?.error;
      assert.equal(error?.message, 'Request tools/call timed out after 1000ms');
      const state = await readState(stateFile);
      assert.equal(alive(state.pid), false);
    } finally {
      await engine?.close().catch(() => {});
      if (originalRequestTimeout === undefined) delete process.env.REQUEST_TIMEOUT;
      else process.env.REQUEST_TIMEOUT = originalRequestTimeout;
    }
  });
});

test('ProbeAgent leaves an unconfigured Codex request at the standalone 10-minute default', async () => {
  await withFakeCodex(async ({ root }) => {
    const originalRequestTimeout = process.env.REQUEST_TIMEOUT;
    delete process.env.REQUEST_TIMEOUT;
    let agent;
    const originalSetTimeout = globalThis.setTimeout;
    const delays = [];
    globalThis.setTimeout = (callback, delay, ...args) => {
      delays.push(delay);
      return originalSetTimeout(callback, delay, ...args);
    };
    try {
      agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [], disableSkills: true });
      await agent.getEngine();
      assert.equal(delays.includes(600000), true);
    } finally {
      globalThis.setTimeout = originalSetTimeout;
      await agent?.cleanup().catch(() => {});
      if (originalRequestTimeout === undefined) delete process.env.REQUEST_TIMEOUT;
      else process.env.REQUEST_TIMEOUT = originalRequestTimeout;
    }
  });
});

test('standalone Codex engine keeps its 10-minute default request timeout', async () => {
  await withFakeCodex(async () => {
    let engine;
    const originalSetTimeout = globalThis.setTimeout;
    const delays = [];
    globalThis.setTimeout = (callback, delay, ...args) => {
      delays.push(delay);
      return originalSetTimeout(callback, delay, ...args);
    };
    try {
      engine = await createCodexEngine();
      assert.equal(delays.includes(600000), true);
    } finally {
      globalThis.setTimeout = originalSetTimeout;
      await engine?.close().catch(() => {});
    }
  });
});

test('invalid direct Codex request timeouts fall back to the 10-minute default', async () => {
  await withFakeCodex(async () => {
    for (const invalidTimeout of [999, 3600001, 1000.5, '1000', null, NaN, Infinity]) {
      let engine;
      const originalSetTimeout = globalThis.setTimeout;
      const delays = [];
      globalThis.setTimeout = (callback, delay, ...args) => {
        delays.push(delay);
        return originalSetTimeout(callback, delay, ...args);
      };
      try {
        engine = await createCodexEngine({ requestTimeout: invalidTimeout });
        assert.equal(delays.includes(600000), true, String(invalidTimeout));
      } finally {
        globalThis.setTimeout = originalSetTimeout;
        await engine?.close().catch(() => {});
      }
    }
  });
});

test('ordinary successful ProbeAgent queries retain reuse until agent cleanup', async () => {
  await withFakeCodex(async ({ root, stateFile }) => {
    const agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [], disableSkills: true, requestTimeout: 1000 });
    const engine = await agent.getEngine();
    try {
      assert.deepEqual((await collect(engine.query('first'))).find(chunk => chunk.type === 'text')?.content, 'ok');
      const stateAfterFirst = await readState(stateFile);
      assert.equal(alive(stateAfterFirst.pid), true);
      assert.deepEqual((await collect(engine.query('second'))).find(chunk => chunk.type === 'text')?.content, 'ok');
      const stateAfterSecond = await readState(stateFile);
      assert.equal(stateAfterSecond.requests.filter(request => request.method === 'tools/call').length, 2);
    } finally {
      await agent.cleanup();
      const stateAfterCleanup = await readState(stateFile);
      assert.equal(alive(stateAfterCleanup.pid), false);
    }
  });
});
