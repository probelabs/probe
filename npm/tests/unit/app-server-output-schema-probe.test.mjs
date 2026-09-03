import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import test from 'node:test';

const codex = process.env.CODEX_BIN || 'codex';

function rpc(child, request) {
  child.stdin.write(`${JSON.stringify(request)}\n`);
  return new Promise((resolve, reject) => {
    let carry = '';
    const onData = (chunk) => {
      carry += chunk;
      const lines = carry.split('\n');
      carry = lines.pop();
      for (const line of lines) {
        if (!line.trim()) continue;
        let message;
        try { message = JSON.parse(line); } catch (error) {
          reject(new Error(`app-server emitted non-JSON output: ${line}`, { cause: error }));
          return;
        }
        if (message.id === request.id) {
          child.stdout.off('data', onData);
          resolve(message);
          return;
        }
      }
    };
    child.stdout.on('data', onData);
    child.once('error', reject);
    child.once('exit', (code, signal) => reject(new Error(`app-server exited before response (code=${code}, signal=${signal})`)));
  });
}

function dynamicTool(name) {
  return { type: 'function', name, description: `probe ${name}`, inputSchema: { type: 'object' } };
}

test('app-server handshake and generated schema expose output-schema fields', async () => {
  const root = await mkdtemp(join(tmpdir(), 'probe-app-server-output-schema-'));
  const subject = await mkdtemp(join(root, 'subject-'));
  const codexHome = join(root, 'codex-home');
  await mkdir(codexHome);
  const schema = join(root, 'schema');
  const child = spawn(codex, ['app-server', '--listen', 'stdio://'], {
    cwd: subject,
    stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, CODEX_HOME: codexHome },
  });
  try {
    const initialize = await rpc(child, {
      id: 1,
      method: 'initialize',
      params: { clientInfo: { name: 'output-schema-probe', version: '0.0.1' }, capabilities: { experimentalApi: true } },
    });
    assert.ok(initialize.result, JSON.stringify(initialize));

    const started = await rpc(child, {
      id: 2,
      method: 'thread/start',
      params: {
        model: 'gpt-5.6-luna', modelProvider: 'openai', config: { model_reasoning_effort: 'xhigh' },
        cwd: subject, approvalPolicy: 'never', sandbox: 'read-only', ephemeral: true,
        dynamicTools: ['search', 'extract', 'listFiles'].map(dynamicTool),
      },
    });
    assert.ok(started.result, JSON.stringify(started));
    const response = started.result;
    assert.equal(response.model, 'gpt-5.6-luna');
    assert.equal(response.modelProvider, 'openai');
    assert.equal(response.reasoningEffort, 'xhigh');
    assert.equal(response.approvalPolicy, 'never');
    assert.equal(response.cwd, subject);
    assert.deepEqual(response.sandbox, { type: 'readOnly', networkAccess: false });
    if ('activePermissionProfile' in response) assert.ok(response.activePermissionProfile === null || typeof response.activePermissionProfile === 'object');

    const generated = spawn(codex, ['app-server', 'generate-json-schema', '--experimental', '--out', schema], { stdio: ['ignore', 'pipe', 'pipe'] });
    const generatedExit = await new Promise((resolve, reject) => {
      let stderr = '';
      generated.stderr.on('data', (chunk) => { stderr += chunk; });
      generated.once('error', reject);
      generated.once('close', (code) => resolve({ code, stderr }));
    });
    assert.equal(generatedExit.code, 0, generatedExit.stderr);
    const turnSchema = JSON.parse(await readFile(join(schema, 'v2', 'TurnStartParams.json'), 'utf8'));
    const threadSchema = JSON.parse(await readFile(join(schema, 'v2', 'ThreadStartParams.json'), 'utf8'));
    assert.ok(turnSchema.properties.outputSchema, 'v2 TurnStartParams.outputSchema missing');
    assert.ok(threadSchema.properties.dynamicTools, 'v2 ThreadStartParams.dynamicTools missing');
  } finally {
    child.stdin.end();
    await new Promise((resolve) => child.once('close', resolve));
    await rm(root, { recursive: true, force: true });
  }
  await assert.rejects(() => readFile(root), /ENOENT/);
});
