import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ProbeAgent } from '@probelabs/probe/agent';
import { previewGovernedCodexInitialDispatch } from '../../src/agent/engines/codex.js';

const packageRoot = fileURLToPath(new URL('../..', import.meta.url));
const tools = ['search', 'extract', 'listFiles'];
const schema = JSON.stringify({ type: 'object', additionalProperties: false, required: ['ok'], properties: { ok: { const: true } } });
const invocationDigest = `sha256:${'a'.repeat(64)}`;
const profile = cwd => ({
  version: 'probe.governed-codex-profile/v1', profileId: 'luna-xhigh-readonly-v1', engine: 'codex',
  model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never',
  cwd, probeTools: [...tools], fallback: false, retries: 0
});

function consumer() {
  const directory = mkdtempSync(join(tmpdir(), 'probe-dispatch-consumer-'));
  const link = join(directory, 'node_modules', '@probelabs', 'probe');
  mkdirSync(dirname(link), { recursive: true });
  symlinkSync(packageRoot, link, 'dir');
  writeFileSync(join(directory, 'package.json'), '{"type":"module"}\n');
  return directory;
}

function governedAgent(cwd, options = {}) {
  return new ProbeAgent({ provider: 'codex', path: cwd, cwd, allowedTools: [...tools],
    governedCodexProfile: profile(cwd), disableMermaidValidation: true, ...options });
}

test('agent subpath has no dotenv/root side effect while the legacy root retains it', () => {
  const directory = consumer();
  const sentinel = 'PROBE_AGENT_SUBPATH_DOTENV_SENTINEL';
  try {
    writeFileSync(join(directory, '.env'), `${sentinel}=loaded\n`);
    const run = specifier => {
      const env = { ...process.env }; delete env[sentinel];
      return spawnSync(process.execPath, ['--input-type=module', '--eval',
        `await import(${JSON.stringify(specifier)}); process.stdout.write(process.env.${sentinel} ?? 'absent')`],
      { cwd: directory, env, encoding: 'utf8' });
    };
    const agent = run('@probelabs/probe/agent');
    assert.equal(agent.status, 0, agent.stderr); assert.equal(agent.stdout, 'absent');
    const root = run('@probelabs/probe');
    assert.equal(root.status, 0, root.stderr); assert.equal(root.stdout, 'loaded');
    const source = readFileSync(join(packageRoot, 'src/agent/ProbeAgent.js'), 'utf8');
    assert.doesNotMatch(source, /dotenv|from ['"]\.\.\/index\.js['"]/);
    assert.match(source, /from ['"]\.\.\/utils\/file-lister\.js['"]/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('preview is frozen, acquires nothing, and equals the subsequent fake runtime dispatch', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'probe-dispatch-preview-'));
  try {
    writeFileSync(join(directory, 'AGENTS.md'), 'Repository dispatch guidance A.\n');
    const agent = governedAgent(directory);
    let acquisitions = 0;
    agent.getEngine = async () => {
      acquisitions++;
      return {
        async *query(prompt, options) {
          const systemPrompt = await agent._getCachedCodexNativeSystemPrompt();
          const dispatch = previewGovernedCodexInitialDispatch({ systemPrompt, customPrompt: agent.customPrompt, prompt });
          yield { type: 'text', content: '{"ok":true}' };
          yield { type: 'metadata', data: { attestation: {
            version: 'probe.governed-codex-attestation/v2', executionContext: { source: 'caller', invocationDigest: options.invocationDigest }, dispatch
          } } };
        },
        async close() {}
      };
    };
    const preview = await agent.previewGovernedAnswerDispatch('review alpha', { schema });
    assert.equal(acquisitions, 0);
    assert.deepEqual(Object.keys(preview), ['source', 'tool', 'promptDigest', 'promptBytes']);
    assert.equal(Object.isFrozen(preview), true);
    assert.deepEqual([preview.source, preview.tool], ['probe-host-tools-call', 'codex']);
    assert.match(preview.promptDigest, /^sha256:[0-9a-f]{64}$/); assert.ok(preview.promptBytes > 0);

    const result = await agent.answerGoverned('review alpha', { schema, invocationDigest });
    assert.equal(acquisitions, 1);
    assert.deepEqual(result.runtimeAttestation.dispatch, preview);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('message, schema, system prompt, and repository guidance bind preview while one agent caches its prompt', async () => {
  const firstDirectory = mkdtempSync(join(tmpdir(), 'probe-dispatch-first-'));
  const secondDirectory = mkdtempSync(join(tmpdir(), 'probe-dispatch-second-'));
  try {
    writeFileSync(join(firstDirectory, 'AGENTS.md'), 'Repository dispatch guidance A.\n');
    writeFileSync(join(secondDirectory, 'AGENTS.md'), 'Repository dispatch guidance B with distinct bytes.\n');
    const first = governedAgent(firstDirectory);
    first.getEngine = async () => { throw new Error('preview must not acquire'); };
    const baseline = await first.previewGovernedAnswerDispatch('review alpha', { schema });
    const messageChanged = await first.previewGovernedAnswerDispatch('review beta', { schema });
    const schemaChanged = await first.previewGovernedAnswerDispatch('review alpha', { schema: JSON.stringify({ type: 'boolean' }) });
    writeFileSync(join(firstDirectory, 'AGENTS.md'), 'changed after cache\n');
    const cached = await first.previewGovernedAnswerDispatch('review alpha', { schema });
    const custom = await governedAgent(firstDirectory, { systemPrompt: 'Distinct governed system prompt.' })
      .previewGovernedAnswerDispatch('review alpha', { schema });
    const repositoryChanged = await governedAgent(secondDirectory).previewGovernedAnswerDispatch('review alpha', { schema });
    assert.notEqual(messageChanged.promptDigest, baseline.promptDigest);
    assert.notEqual(schemaChanged.promptDigest, baseline.promptDigest);
    assert.notEqual(custom.promptDigest, baseline.promptDigest);
    assert.notEqual(repositoryChanged.promptDigest, baseline.promptDigest);
    assert.deepEqual(cached, baseline);
  } finally {
    rmSync(firstDirectory, { recursive: true, force: true });
    rmSync(secondDirectory, { recursive: true, force: true });
  }
});

test('public declarations expose only the dispatch receipt, not prompt bytes', () => {
  const directory = consumer();
  try {
    writeFileSync(join(directory, 'consumer.ts'), `
      import { ProbeAgent, type GovernedAnswerDispatch } from '@probelabs/probe/agent';
      declare const agent: ProbeAgent;
      const pending: Promise<Readonly<GovernedAnswerDispatch>> = agent.previewGovernedAnswerDispatch('x', { schema: '{"type":"boolean"}' });
      pending.then(value => { const digest: \`sha256:\${string}\` = value.promptDigest; void digest; });
      // @ts-expect-error preview never returns prompt bytes
      pending.then(value => value.prompt);
    `);
    writeFileSync(join(directory, 'tsconfig.json'), JSON.stringify({ compilerOptions: {
      target: 'ES2022', module: 'NodeNext', moduleResolution: 'NodeNext', strict: true, noEmit: true, skipLibCheck: true
    }, files: ['consumer.ts'] }));
    const compile = spawnSync(process.execPath, [join(packageRoot, 'node_modules', 'typescript', 'bin', 'tsc'), '-p', join(directory, 'tsconfig.json')], { encoding: 'utf8' });
    assert.equal(compile.status, 0, `${compile.stdout}${compile.stderr}`);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
