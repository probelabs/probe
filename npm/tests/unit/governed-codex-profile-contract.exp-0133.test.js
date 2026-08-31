import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import * as kernel from '../../src/agent/engines/governed-codex-profile.js';

const { validateGovernedCodexProfile, buildGovernedCodexInitialToolArgs, attestGovernedCodexSession } = kernel;
const cwd = realpathSync(process.cwd());
const mcp = { name: 'probe_0123456789abcdef', url: 'http://127.0.0.1:43123/mcp' };
const rollout = '/ENVIRONMENT_SENTINEL/HOME_SENTINEL/sessions/2026/08/25/rollout-2026-08-25T12-34-56-01234567-89ab-cdef-0123-456789abcdef.jsonl';
const toolNames = ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'];
const featureNames = [
  'shell_tool', 'multi_agent', 'multi_agent_v2', 'enable_fanout', 'apps', 'enable_mcp_apps',
  'tool_suggest', 'plugins', 'in_app_browser', 'browser_use', 'browser_use_full_cdp_access',
  'browser_use_external', 'computer_use', 'remote_plugin', 'plugin_sharing', 'image_generation',
  'skill_mcp_dependency_install', 'hooks', 'request_permissions_tool', 'standalone_web_search',
];

function profile() {
  return {
    version: 'probe.governed-codex-profile/v1', profileId: 'luna-xhigh-readonly-v1', engine: 'codex',
    model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never',
    cwd, probeTools: ['search', 'extract', 'listFiles'], fallback: false, retries: 0,
  };
}
function permission() {
  return { type: 'managed', file_system: { type: 'restricted', entries: [{ access: 'read', path: { type: 'special', value: { kind: 'root' } } }] }, network: 'restricted' };
}
function captured(id = 'thread-1') {
  return {
    jsonrpc: '2.0', method: 'codex/event',
    params: {
      _meta: { requestId: 2, threadId: id }, id: '',
      msg: {
        type: 'session_configured', session_id: id, thread_id: id, model: 'gpt-5.6-luna',
        model_provider_id: 'openai', approval_policy: 'never', approvals_reviewer: 'user',
        permission_profile: permission(), reasoning_effort: 'xhigh', rollout_path: rollout, cwd,
      },
    },
  };
}
function clone(value) { return JSON.parse(JSON.stringify(value)); }
function frozen(value) {
  if (!value || typeof value !== 'object') return;
  assert.equal(Object.isFrozen(value), true);
  for (const child of Object.values(value)) frozen(child);
}
function canonical(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
}
function digest(value) { return createHash('sha256').update(canonical(value), 'utf8').digest('hex'); }
function rejected(fn) { assert.throws(fn, { name: 'TypeError' }); }

test('exact profile, request, and captured event produce deterministic frozen values', () => {
  assert.deepEqual(Object.keys(kernel).sort(), ['attestGovernedCodexSession', 'buildGovernedCodexInitialToolArgs', 'validateGovernedCodexProfile']);
  const normalized = validateGovernedCodexProfile(profile());
  assert.deepEqual(normalized, profile());
  assert.notEqual(normalized, profile());
  frozen(normalized);
  const prompt = 'PROMPT_SENTINEL SOURCE_SENTINEL CANDIDATE_SENTINEL';
  const args = buildGovernedCodexInitialToolArgs({ profile: profile(), prompt, mcp: { ...mcp } });
  const features = Object.fromEntries(featureNames.map((name) => [name, false]));
  const tools = Object.fromEntries(toolNames.map((name) => [name, { approval_mode: 'approve' }]));
  assert.deepEqual(args, {
    prompt, model: 'gpt-5.6-luna',
    config: {
      model_reasoning_effort: 'xhigh', web_search: 'disabled', features, skills: { include_instructions: false },
      mcp_servers: { [mcp.name]: { url: mcp.url, default_tools_approval_mode: 'prompt', enabled_tools: toolNames, tools } },
    },
    cwd, sandbox: 'read-only', 'approval-policy': 'never',
  });
  assert.deepEqual(Object.keys(args), ['prompt', 'model', 'config', 'cwd', 'sandbox', 'approval-policy']);
  frozen(args);
  const attestation = attestGovernedCodexSession({ profile: profile(), events: [captured()] });
  assert.deepEqual(attestation, {
    version: 'probe.governed-codex-attestation/v1', profileId: 'luna-xhigh-readonly-v1',
    requested: { profileDigest: digest(normalized), cwdDigest: digest(cwd), probeToolsDigest: digest(normalized.probeTools), model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never' },
    observed: { source: 'session_configured', model: 'gpt-5.6-luna', modelProviderId: 'openai', reasoningEffort: 'xhigh', approvalPolicy: 'never', cwdDigest: digest(cwd), permissionProfileDigest: digest(permission()), filesystem: 'restricted-read-root', network: 'restricted' },
    correlation: { requestId: 2, eventCount: 1 }, usage: { status: 'unavailable' },
  });
  frozen(attestation);
  const argsAgain = buildGovernedCodexInitialToolArgs({ profile: profile(), prompt, mcp: { ...mcp } });
  const attestationAgain = attestGovernedCodexSession({ profile: profile(), events: [captured()] });
  assert.equal(canonical(argsAgain), canonical(args));
  assert.equal(canonical(attestationAgain), canonical(attestation));
});

test('profile validation rejects missing, unknown, aliased, typed, widened, tool-order, and cwd defects', () => {
  const mutations = [
    (p) => { delete p.model; }, (p) => { p.unknown = true; },
    (p) => { p.reasoning_effort = p.reasoningEffort; delete p.reasoningEffort; },
    (p) => { p.retries = '0'; }, (p) => { p.fallback = true; }, (p) => { p.sandbox = 'workspace-write'; },
    (p) => { p.probeTools.reverse(); }, (p) => { p.probeTools.push('write'); },
    (p) => { p.cwd = 'relative'; }, (p) => { p.cwd = `${cwd}/.exp-0133-definitely-absent`; },
    (p) => { p.cwd = fileURLToPath(import.meta.url); }, (p) => { p.cwd = `${cwd}\0bad`; },
  ];
  for (const mutate of mutations) { const value = profile(); mutate(value); rejected(() => validateGovernedCodexProfile(value)); }
  const getter = profile(); Object.defineProperty(getter, 'model', { enumerable: true, get: () => 'gpt-5.6-luna' });
  rejected(() => validateGovernedCodexProfile(getter));
});

test('request compiler rejects input, prompt, and loopback MCP widening', () => {
  rejected(() => buildGovernedCodexInitialToolArgs({ profile: profile(), prompt: 'x' }));
  rejected(() => buildGovernedCodexInitialToolArgs({ profile: profile(), prompt: 'x', mcp, extra: true }));
  rejected(() => buildGovernedCodexInitialToolArgs({ profile: profile(), prompt: '', mcp }));
  rejected(() => buildGovernedCodexInitialToolArgs({ profile: profile(), prompt: 'é'.repeat(65537), mcp }));
  const badBindings = [
    { ...mcp, extra: true }, { ...mcp, name: 'probe_search' }, { ...mcp, url: 'not a url' },
    { ...mcp, url: 'https://127.0.0.1:43123/mcp' }, { ...mcp, url: 'http://localhost:43123/mcp' },
    { ...mcp, url: 'http://127.0.0.1:0/mcp' }, { ...mcp, url: 'http://127.0.0.1:65536/mcp' },
    { ...mcp, url: 'http://user@127.0.0.1:43123/mcp' }, { ...mcp, url: 'http://127.0.0.1:43123/other' },
    { ...mcp, url: 'http://127.0.0.1:43123/mcp?q=1' }, { ...mcp, url: 'http://127.0.0.1:43123/mcp#x' },
  ];
  for (const binding of badBindings) rejected(() => buildGovernedCodexInitialToolArgs({ profile: profile(), prompt: 'x', mcp: binding }));
});

test('attester rejects correlation, identity, rollout, and recursive captured-shape defects', () => {
  rejected(() => attestGovernedCodexSession({ profile: profile(), events: [] }));
  rejected(() => attestGovernedCodexSession({ profile: profile(), events: [captured(), captured()] }));
  rejected(() => attestGovernedCodexSession({ profile: profile(), events: [captured()], extra: true }));
  const mutations = [
    (e) => { e.jsonrpc = '1.0'; }, (e) => { e.method = 'other'; }, (e) => { e.params.id = 'x'; },
    (e) => { e.params._meta.requestId = 3; }, (e) => { e.params.msg.type = 'turn_started'; },
    (e) => { e.params.msg.thread_id = 'other'; }, (e) => { e.params._meta.threadId = 'unsafe id'; },
    (e) => { e.extra = true; }, (e) => { e.params._meta.extra = true; }, (e) => { delete e.params.msg.model; },
    (e) => { e.params.msg.permission_profile.file_system.entries[0].path.value.extra = true; },
    (e) => { e.params.msg.rollout_path = rollout.replace('/08/25/', '/08/24/'); },
    (e) => { e.params.msg.rollout_path = `${rollout}/extra`; },
    (e) => { e.params.msg.rollout_path = `${rollout}\n`; }, (e) => { e.params.msg.rollout_path = `${rollout}\r`; },
    (e) => { e.params.msg.rollout_path = `${rollout}\u2028`; }, (e) => { e.params.msg.rollout_path = `${rollout}\u2029`; },
  ];
  for (const mutate of mutations) { const event = captured(); mutate(event); rejected(() => attestGovernedCodexSession({ profile: profile(), events: [event] })); }
});

test('attester rejects observed model, effort, approval, cwd, permission, filesystem, and network mismatch', () => {
  const mutations = [
    (m) => { m.model = 'other'; }, (m) => { m.model_provider_id = 'other'; },
    (m) => { m.reasoning_effort = 'high'; }, (m) => { m.approval_policy = 'on-request'; },
    (m) => { m.approvals_reviewer = 'agent'; }, (m) => { m.cwd = '/'; },
    (m) => { m.permission_profile.type = 'unmanaged'; }, (m) => { m.permission_profile.network = 'enabled'; },
    (m) => { m.permission_profile.file_system.type = 'full'; },
    (m) => { m.permission_profile.file_system.entries.push(clone(m.permission_profile.file_system.entries[0])); },
    (m) => { m.permission_profile.file_system.entries[0].access = 'write'; },
    (m) => { m.permission_profile.file_system.entries[0].path.type = 'literal'; },
    (m) => { m.permission_profile.file_system.entries[0].path.value.kind = 'cwd'; },
  ];
  for (const mutate of mutations) { const event = captured(); mutate(event.params.msg); rejected(() => attestGovernedCodexSession({ profile: profile(), events: [event] })); }
});

test('sanitized attestation leaks no prompt, path, identity, environment, credential, usage zero, or raw event', () => {
  const prompt = 'PROMPT_SENTINEL SOURCE_SENTINEL CANDIDATE_SENTINEL';
  buildGovernedCodexInitialToolArgs({ profile: profile(), prompt, mcp });
  const raw = captured('credential_SENTINEL');
  const serialized = JSON.stringify(attestGovernedCodexSession({ profile: profile(), events: [raw] }));
  for (const sentinel of [...prompt.split(' '), mcp.url, cwd, rollout, 'credential_SENTINEL', 'ENVIRONMENT_SENTINEL', 'HOME_SENTINEL']) assert.equal(serialized.includes(sentinel), false, sentinel);
  for (const forbidden of ['rollout_path', 'session_id', 'thread_id', 'permission_profile', 'prompt', 'mcp', 'credential', 'environment', '"tokens":0', '"cost":0']) assert.equal(serialized.toLowerCase().includes(forbidden), false, forbidden);
  assert.deepEqual(JSON.parse(serialized).usage, { status: 'unavailable' });
});

test('module side-effect census is Node built-ins plus read-only filesystem inspection only', () => {
  const source = readFileSync(new URL('../../src/agent/engines/governed-codex-profile.js', import.meta.url), 'utf8');
  function assertSideEffectCensus(candidateSource) {
    const staticImports = candidateSource.split('\n').filter((line) => line.startsWith('import '));
    assert.deepEqual(staticImports, ["import { createHash } from 'node:crypto';", "import { realpathSync, statSync } from 'node:fs';", "import { isAbsolute, normalize } from 'node:path';"]);
    assert.equal((candidateSource.match(/\bimport\s+(?!\()/g) || []).length, 3);
    assert.deepEqual([...new Set(candidateSource.match(/\b(?:Buffer|URL)\b/g) || [])].sort(), ['Buffer', 'URL']);
    assert.doesNotMatch(candidateSource, /import\s*\(|\b(?:require|eval|Function|process|globalThis)\b/);
    assert.doesNotMatch(candidateSource, /\b(?:spawn|fork|fetch|createServer|connect|writeFile(?:Sync)?|appendFile(?:Sync)?|createWriteStream|rm(?:Sync)?|unlink(?:Sync)?|rename(?:Sync)?|mkdir(?:Sync)?|copyFile(?:Sync)?|setTimeout|setInterval|setImmediate|ProbeAgent)\b/);
  }
  assertSideEffectCensus(source);
  assert.throws(() => assertSideEffectCensus(`${source}\nimport { exec } from 'node:child_process';`));
  assert.throws(() => assertSideEffectCensus(`${source}\nvoid import('node:child_process');`));
  assert.throws(() => assertSideEffectCensus(`${source}\nrequire('node:child_process');`));
});
