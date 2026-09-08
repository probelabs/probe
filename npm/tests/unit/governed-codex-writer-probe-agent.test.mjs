import test from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs } from '../../src/agent/engines/governed-codex-profile.js';
import { createCodexEngine } from '../../src/agent/engines/codex.js';

const TOOLS = ['search', 'extract', 'listFiles'];
const PROFILE_ID = 'luna-xhigh-isolated-writer-v1';
const SCHEMA = JSON.stringify({ type: 'string' });

function rawExecFailure(code = 'GOVERNED_CODEX_EXEC_ITEM') {
  const error = new Error('secret raw provider payload');
  Object.assign(error, {
    code,
    hostile: { token: 'must-not-cross-the-boundary' },
    event: {
        source: 'codex-exec-rejected-item/v1',
        predicate: 'item_status',
        eventType: 'item.completed',
        itemType: 'command_execution',
        itemStatus: 'failed',
        eventFields: [
          { name: 'item', type: 'object' },
          { name: 'type', type: 'string', size: 17 },
        ],
        itemFields: [
          { name: 'command', type: 'string', size: 19 },
          { name: 'status', type: 'string', size: 6 },
        ],
    },
  });
  return error;
}

function governedAnswerAgent(engine) {
  const cwd = realpathSync(process.cwd());
  const profile = writerProfile(cwd);
  const agent = new ProbeAgent({ provider: 'codex', path: cwd, cwd, allowedTools: [...TOOLS],
    governedCodexProfile: profile, governedCodexTransport: 'exec-jsonl-default-auth-v1',
    codexBin: '/usr/local/bin/codex', codexSha256: 'a'.repeat(64), disableMermaidValidation: true });
  agent.getSystemMessage = async () => '';
  agent.engine = engine;
  return agent;
}

function legacyGovernedAnswerAgent(engine) {
  const cwd = realpathSync(process.cwd());
  const profile = writerProfile(cwd);
  const agent = new ProbeAgent({ provider: 'codex', path: cwd, cwd, allowedTools: [...TOOLS],
    governedCodexProfile: profile, governedCodexTransport: 'mcp-server-v1', disableMermaidValidation: true });
  agent.getSystemMessage = async () => '';
  agent.engine = engine;
  return agent;
}

function writerProfile(cwd) {
  return {
    version: 'probe.governed-codex-profile/v3', profileId: PROFILE_ID, engine: 'codex',
    model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'workspace-write',
    approvalPolicy: 'never', cwd, probeMcpTools: [...TOOLS],
    codexNativeTools: ['apply_patch', 'exec'], fallback: false, retries: 0,
  };
}

function sessionEvent(cwd) {
  const sessionId = 'writer-session';
  return {
    jsonrpc: '2.0', method: 'codex/event',
    params: {
      _meta: { requestId: 2, threadId: sessionId }, id: '',
      msg: {
        type: 'session_configured', session_id: sessionId, thread_id: sessionId,
        model: 'gpt-5.6-luna', model_provider_id: 'openai', approval_policy: 'never',
        approvals_reviewer: 'user', reasoning_effort: 'xhigh',
        rollout_path: `${cwd}/sessions/2026/09/07/rollout-2026-09-07T12-00-00-00000000-0000-4000-8000-000000000001.jsonl`,
        cwd,
        permission_profile: {
          type: 'managed',
          file_system: { type: 'restricted', entries: [
            { access: 'read', path: { type: 'special', value: { kind: 'root' } } },
            { access: 'write', path: { type: 'path', path: cwd } },
            { access: 'read', missing_path_behavior: 'skip', path: { type: 'path', path: `${cwd}/.git` } },
            { access: 'read', missing_path_behavior: 'skip', path: { type: 'path', path: `${cwd}/.agents` } },
            { access: 'read', missing_path_behavior: 'skip', path: { type: 'path', path: `${cwd}/.codex` } },
          ] },
          network: 'restricted',
        },
      },
    },
  };
}

function nativeAggregate() {
  return {
    total: 2,
    tools: [
      { name: 'apply_patch', status: 'completed', count: 1 },
      { name: 'exec', status: 'completed', count: 1 },
    ],
  };
}

test('writer profile admits native apply_patch and exec through ProbeAgent', async () => {
  const cwd = realpathSync(process.cwd());
  const profile = writerProfile(cwd);
  const aggregate = nativeAggregate();
  const receipt = attestGovernedCodexSession({ profile, events: [sessionEvent(cwd), aggregate] });
  const agent = new ProbeAgent({ provider: 'codex', path: cwd, cwd, allowedTools: [...TOOLS],
    governedCodexProfile: profile, disableMermaidValidation: true });
  const toolEvents = [];
  agent.events.on('toolCall', event => toolEvents.push(event));
  let closed = 0;
  agent.engine = {
    async *query() {
      yield { type: 'text', content: JSON.stringify('writer-ok') };
      yield { type: 'metadata', data: { attestation: receipt } };
      yield { type: 'toolBatch', total: aggregate.total, tools: aggregate.tools.map(item => ({ ...item })) };
    },
    async close() { closed++; },
  };

  const answer = await agent.answerGoverned('write the isolated fixture', { schema: SCHEMA });
  assert.equal(answer.data, 'writer-ok');
  assert.equal(answer.runtimeAttestation.profileId, PROFILE_ID);
  assert.equal(answer.runtimeAttestation.observed.filesystem, 'restricted-write-cwd');
  assert.deepEqual(answer.runtimeAttestation.observed.nativeTools, aggregate);
  assert.deepEqual(toolEvents, aggregate.tools);
  assert.equal(closed, 1);
});

test('writer attestation rejects temp roots and every permission-scope mutation', () => {
  const cwd = realpathSync(process.cwd());
  const profile = writerProfile(cwd);
  const aggregate = nativeAggregate();
  const mutations = [
    event => { event.params.msg.permission_profile.file_system.entries[1].path = { type: 'special', value: { kind: 'tmpdir' } }; },
    event => { event.params.msg.permission_profile.file_system.entries[1].path = { type: 'special', value: { kind: 'slash_tmp' } }; },
    event => { event.params.msg.permission_profile.file_system.entries.push({ access: 'read', missing_path_behavior: 'skip', path: { type: 'path', path: `${cwd}/extra` } }); },
    event => { event.params.msg.permission_profile.file_system.entries[1].path.path = `${cwd}/other`; },
    event => { event.params.msg.permission_profile.file_system.entries.reverse(); },
    event => { event.params.msg.permission_profile.file_system.entries[0].access = 'write'; },
    event => { event.params.msg.permission_profile.file_system.entries[2].path.path = `${cwd}/.other`; },
    event => { event.params.msg.permission_profile.file_system.entries[2].missing_path_behavior = 'error'; },
    event => { event.params.msg.permission_profile.file_system.entries[2].access = 'write'; },
    event => { event.params.msg.permission_profile.file_system.entries[2].path.type = 'special'; },
    event => { event.params.msg.permission_profile.file_system.entries[2].path = { type: 'path', path: `${cwd}/.git/` }; },
  ];
  for (const mutate of mutations) {
    const event = sessionEvent(cwd);
    mutate(event);
    assert.throws(() => attestGovernedCodexSession({ profile, events: [event, aggregate] }), TypeError);
  }
});

test('ordinary ProbeAgent.answer consumes writer native evidence and closes', async () => {
  const cwd = realpathSync(process.cwd());
  const profile = writerProfile(cwd);
  const aggregate = nativeAggregate();
  const receipt = attestGovernedCodexSession({ profile, events: [sessionEvent(cwd), aggregate] });
  const agent = new ProbeAgent({ provider: 'codex', path: cwd, cwd, allowedTools: [...TOOLS],
    governedCodexProfile: profile, disableMermaidValidation: true });
  const toolEvents = [];
  agent.events.on('toolCall', event => toolEvents.push(event));
  let closed = 0;
  agent.engine = {
    async *query() {
      yield { type: 'text', content: 'ordinary-writer-ok' };
      yield { type: 'metadata', data: { attestation: receipt } };
      yield { type: 'toolBatch', total: aggregate.total, tools: aggregate.tools.map(item => ({ ...item })) };
    },
    async close() { closed++; },
  };
  const answer = await agent.answer('use the isolated writer');
  assert.equal(answer, 'ordinary-writer-ok');
  assert.deepEqual(toolEvents, aggregate.tools);
  assert.equal(closed, 1);
});

test('governed answer preserves a closed exec query diagnostic and closes once', async () => {
  let closed = 0;
  const queryError = rawExecFailure();
  const agent = governedAnswerAgent({
    async *query() { throw queryError; },
    async close() { closed++; },
  });

  const failure = await agent.answer('query failure').then(() => null, error => error);
  assert.equal(failure?.name, 'GovernedAnswerFailure');
  assert.equal(failure?.answerFailureStage, 'provider_engine');
  assert.equal(failure?.providerEngineFailureBoundary, 'query');
  assert.equal(Object.isFrozen(failure), true);
  assert.deepEqual(failure?.providerEngineDiagnostic, {
    version: 'probe.governed-codex-exec-failure/v1',
    code: 'GOVERNED_CODEX_EXEC_ITEM',
    event: {
      source: 'codex-exec-rejected-item/v1',
      predicate: 'item_status',
      eventType: 'item.completed',
      itemType: 'command_execution',
      itemStatus: 'failed',
      eventFields: [
        { name: 'item', type: 'object' },
        { name: 'type', type: 'string', size: 17 },
      ],
      itemFields: [
        { name: 'command', type: 'string', size: 19 },
        { name: 'status', type: 'string', size: 6 },
      ],
    },
  });
  assert.equal(Object.prototype.hasOwnProperty.call(failure, 'hostile'), false);
  assert.equal(JSON.stringify(failure).includes('must-not-cross-the-boundary'), false);
  assert.equal(failure.message, '');
  assert.equal(closed, 1);
});

test('governed query failure remains authoritative when close also fails', async () => {
  let closed = 0;
  const queryError = rawExecFailure();
  const agent = governedAnswerAgent({
    async *query() { throw queryError; },
    async close() { closed++; throw rawExecFailure('GOVERNED_CODEX_EXEC_EXIT'); },
  });

  const failure = await agent.answer('query and close failure').then(() => null, error => error);
  assert.equal(failure?.providerEngineFailureBoundary, 'query');
  assert.equal(failure?.providerEngineDiagnostic?.code, 'GOVERNED_CODEX_EXEC_ITEM');
  assert.equal(closed, 1);
});

test('governed close-only failure is normalized at the close boundary', async () => {
  let closed = 0;
  const agent = governedAnswerAgent({
    async *query() { yield { type: 'text', content: 'ok' }; },
    async close() { closed++; throw rawExecFailure('GOVERNED_CODEX_EXEC_EXIT'); },
  });

  const failure = await agent.answer('close failure').then(() => null, error => error);
  assert.equal(failure?.name, 'GovernedAnswerFailure');
  assert.equal(failure?.answerFailureStage, 'provider_engine');
  assert.equal(failure?.providerEngineFailureBoundary, 'close');
  assert.equal(failure?.providerEngineDiagnostic?.code, 'GOVERNED_CODEX_EXEC_EXIT');
  assert.equal(closed, 1);
});

test('legacy governed MCP query preserves its raw error and closes directly', async () => {
  const queryError = new Error('legacy query error');
  let closed = 0;
  const agent = legacyGovernedAnswerAgent({
    async *query() { throw queryError; },
    async close() { closed++; },
  });

  const failure = await agent.answer('legacy query failure').then(() => null, error => error);
  assert.equal(failure, queryError);
  assert.equal(closed, 1);
});

test('legacy governed MCP close preserves its raw close error', async () => {
  const closeError = new Error('legacy close error');
  const agent = legacyGovernedAnswerAgent({
    async *query() { yield { type: 'text', content: 'legacy-ok' }; },
    async close() { throw closeError; },
  });

  const failure = await agent.answer('legacy close failure').then(() => null, error => error);
  assert.equal(failure, closeError);
});

test('writer dispatch args are closed before Codex process or MCP startup', async () => {
  const cwd = realpathSync(process.cwd());
  const profile = writerProfile(cwd);
  const args = buildGovernedCodexInitialToolArgs({ profile, prompt: 'bounded writer',
    mcp: { name: 'probe_0123456789abcdef', url: 'http://127.0.0.1:43123/mcp' } });
  assert.equal(args.config.features.shell_tool, true);
  assert.equal(Object.isFrozen(args.config.features), true);
  assert.throws(() => { args.config.features.shell_tool = false; }, TypeError);
  assert.deepEqual(args.config.sandbox_workspace_write, {
    network_access: false, writable_roots: [], exclude_tmpdir_env_var: true, exclude_slash_tmp: true,
  });
  assert.deepEqual(args, {
    prompt: 'bounded writer', model: 'gpt-5.6-luna',
    config: {
      model_reasoning_effort: 'xhigh', web_search: 'disabled',
      features: Object.fromEntries([
        'shell_tool', 'multi_agent', 'multi_agent_v2', 'enable_fanout', 'apps', 'enable_mcp_apps',
        'tool_suggest', 'plugins', 'in_app_browser', 'browser_use', 'browser_use_full_cdp_access',
        'browser_use_external', 'computer_use', 'remote_plugin', 'plugin_sharing', 'image_generation',
        'skill_mcp_dependency_install', 'hooks', 'request_permissions_tool', 'standalone_web_search',
      ].map(name => [name, name === 'shell_tool'])),
      skills: { include_instructions: false },
      sandbox_workspace_write: { network_access: false, writable_roots: [], exclude_tmpdir_env_var: true, exclude_slash_tmp: true },
      mcp_servers: { probe_0123456789abcdef: {
        url: 'http://127.0.0.1:43123/mcp', default_tools_approval_mode: 'prompt',
        enabled_tools: ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'],
        tools: Object.fromEntries(['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'].map(name => [name, { approval_mode: 'approve' }])),
      } },
    },
    cwd, sandbox: 'workspace-write', 'approval-policy': 'never',
  });

  const conflicts = [
    { sandbox: 'read-only' }, { cwd: `${cwd}/other` }, { approvalPolicy: 'on-request' },
    { reasoningEffort: 'high' }, { model: 'other' }, { allowedTools: ['search'] },
    { allowedTools: { mode: 'all', allowed: TOOLS } }, { config: {} }, { env: { CODEX_HOME: '/tmp' } },
    { retry: { maxRetries: 1 } }, { fallback: { auto: true } }, { sessionId: 'reused-session' },
  ];
  for (const override of conflicts) {
    await assert.rejects(() => createCodexEngine({ ...override, governedCodexProfile: profile }), TypeError);
  }
});
