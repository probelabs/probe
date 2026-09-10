import test from 'node:test';
import assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildGovernedCodexExecLaunch, createGovernedCodexExecEngine, normalizeGovernedCodexExecFailure, previewGovernedCodexExecDispatch, projectGovernedCodexExecFailure, validateGovernedCodexExecAttestation } from '../../src/agent/engines/governed-codex-exec.js';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { validateJsonResponse } from '../../src/agent/schemaUtils.js';

function profile(cwd) {
  return {
    version: 'probe.governed-codex-profile/v1', profileId: 'luna-xhigh-readonly-v1', engine: 'codex',
    model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never',
    cwd, probeTools: ['search', 'extract', 'listFiles'], fallback: false, retries: 0,
  };
}

function nativeProfile(cwd, version = 'probe.governed-codex-profile/v2') {
  return {
    version, profileId: version === 'probe.governed-codex-profile/v3'
      ? 'luna-xhigh-isolated-writer-v1' : 'luna-xhigh-readonly-native-exec-v1', engine: 'codex',
    model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: version === 'probe.governed-codex-profile/v3' ? 'workspace-write' : 'read-only',
    approvalPolicy: 'never', cwd, probeMcpTools: ['search', 'extract', 'listFiles'],
    codexNativeTools: version === 'probe.governed-codex-profile/v3' ? ['apply_patch', 'exec'] : ['exec'],
    fallback: false, retries: 0,
  };
}

function agent(cwd) {
  const events = new EventEmitter();
  return {
    allowedTools: { isEnabled: name => ['search', 'extract', 'listFiles'].includes(name) },
    toolImplementations: {
      search: { execute: async () => 'search' },
      extract: { execute: async () => 'extract' },
      listFiles: { execute: async () => 'listFiles' },
    },
    sessionId: 'governed-exec-test', cwd, workspaceRoot: cwd, events,
  };
}

function fixture(events, behavior = 'normal') {
  const root = mkdtempSync(join(tmpdir(), 'probe-governed-exec-'));
  const observed = join(root, 'observed.json');
  const script = join(root, 'fake-codex');
const source = `#!/usr/bin/env node
const fs = require('node:fs');
const events = ${JSON.stringify(events)};
const schemaIndex = process.argv.indexOf('--output-schema');
const schemaPath = schemaIndex >= 0 ? process.argv[schemaIndex + 1] : null;
const schemaBytes = schemaPath ? fs.readFileSync(schemaPath) : null;
const schemaMode = schemaPath ? fs.statSync(schemaPath).mode & 0o777 : null;
fs.writeFileSync(${JSON.stringify(observed)}, JSON.stringify({ args: process.argv.slice(2), codexHome: process.env.CODEX_HOME,
  schemaPath, schemaText: schemaBytes?.toString('utf8') ?? null, schemaMode }));
const mcpUrlArg = process.argv.find(value => /^mcp_servers\\.[^.]+\\.url=/.test(value));
const mcpServerName = mcpUrlArg?.match(/^mcp_servers\\.([^.]+)\\.url=/)?.[1] ?? null;
const mcpUrl = mcpUrlArg ? JSON.parse(mcpUrlArg.slice(mcpUrlArg.indexOf('=') + 1)) : null;
const preparedEvents = events.map(event => event.item?.server === '__MCP_SERVER__'
  ? { ...event, item: { ...event.item, server: mcpServerName } } : event);
(async () => {
  if (process.argv.includes('--version')) { process.stdout.write('codex 0.153.4\\n'); return; }
  ${behavior === 'hang' ? 'setInterval(() => {}, 1000); return;' : ''}
  ${behavior === 'mcp-success' ? `await fetch(mcpUrl.replace(/\\/mcp$/, '/rpc'), { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'mcp__probe__search', arguments: { query: 'dummy' } } }) });` : ''}
  for (const event of preparedEvents) process.stdout.write(JSON.stringify(event) + '\\n');
  ${behavior === 'exit-7' ? 'process.exitCode = 7;' : ''}
})().catch(() => { process.exitCode = 1; });
`;
  writeFileSync(script, source);
  chmodSync(script, 0o700);
  return { root, script, observed };
}

const validEvents = () => [
  { type: 'thread.started', thread_id: 'thread-1' },
  { type: 'turn.started' },
  { type: 'item.completed', item: { id: 'reason-1', type: 'reasoning', text: 'discard-me' } },
  { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
  { type: 'turn.completed', usage: { input_tokens: 12, cached_input_tokens: 4, output_tokens: 3 } },
];

async function withEngine(events, callback, options = {}, behavior = 'normal') {
  const fixtureRoot = fixture(events, behavior);
  try {
    const engine = await createGovernedCodexExecEngine({
      agent: agent(fixtureRoot.root), profile: profile(fixtureRoot.root), prompt: 'bounded prompt',
      codexPath: fixtureRoot.script, codexSha256: `sha256:${createHash('sha256').update(readFileSync(fixtureRoot.script)).digest('hex')}`, timeoutMs: 1000, ...options,
    });
    try { return await callback(engine, fixtureRoot); }
    finally { await engine.close(); }
  } finally {
    rmSync(fixtureRoot.root, { recursive: true, force: true });
  }
}

test('governed exec builds scoped no-shell launch and returns only bounded answer/usage', async () => {
  await withEngine(validEvents(), async (engine, fixtureRoot) => {
    const result = await engine.run();
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    assert.equal(observed.codexHome, undefined);
    assert.deepEqual(observed.args.slice(0, 7), ['exec', '--ignore-user-config', '--ignore-rules', '--ephemeral', '--json', '--model', 'gpt-5.6-luna']);
    assert.equal(observed.args.includes('--sandbox'), true);
    assert.equal(observed.args.includes('--cd'), true);
    assert.equal(observed.args.includes('-c'), true);
    assert.equal(observed.args.some(value => value === 'model_reasoning_effort="xhigh"'), true);
    assert.equal(observed.args.some(value => value === 'approval_policy="never"'), true);
    assert.equal(observed.args.some(value => value.startsWith('mcp_servers.probe_')), true);
    assert.deepEqual(result.answer, '{"ok":true}');
    assert.deepEqual(result.usage, { cached_input_tokens: 4, input_tokens: 12, output_tokens: 3 });
    assert.deepEqual(result.evidence, { eventCount: 5, completedItemCount: 2, agentMessageCount: 1, usedToolItems: [], probeMcpCallCount: 0 });
    assert.equal(validateGovernedCodexExecAttestation(result.attestation), result.attestation);
    assert.equal(result.attestation.enforced.codexHome, 'omitted');
    assert.equal(result.attestation.observed.terminal, 'turn.completed');
    assert.equal(result.process.exitCode, 0);
    assert.equal(Object.prototype.hasOwnProperty.call(result, 'stderr'), false);
    assert.equal(JSON.stringify(result).includes('discard-me'), false);
    assert.equal(JSON.stringify(result).includes('mcp__probe__search'), false);
    await assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_ONE_QUERY/);
  });
});

test('governed exec forwards an explicit output schema as an exact private artifact', async () => {
  const schema = '{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}';
  await withEngine(validEvents(), async (engine, fixtureRoot) => {
    await engine.run();
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    assert.equal(observed.schemaPath, null);
    assert.equal(observed.args.includes('--output-schema'), false);
    assert.equal(observed.args.at(-1), 'bounded prompt');
  }, { timeoutMs: 2000 });

  await withEngine(validEvents(), async (engine, fixtureRoot) => {
    await engine.run(undefined, { schema });
    const launch = engine.launch;
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    const schemaFlag = observed.args.indexOf('--output-schema');
    assert.equal(schemaFlag >= 0, true);
    assert.equal(observed.args[schemaFlag + 1], launch.outputSchemaPath);
    assert.equal(observed.args.at(-3), '--output-schema');
    assert.equal(observed.args.at(-2), launch.outputSchemaPath);
    assert.equal(observed.args.at(-1), 'bounded prompt');
    assert.equal(observed.schemaPath, launch.outputSchemaPath);
    assert.equal(observed.schemaText, schema);
    assert.equal(observed.schemaMode, 0o600);
    assert.equal(readFileSync(launch.outputSchemaPath, 'utf8'), schema);
    assert.equal(launch.outputSchemaBytes, Buffer.byteLength(schema, 'utf8'));
    assert.equal(launch.outputSchemaDigest, `sha256:${createHash('sha256').update(schema).digest('hex')}`);
    const noSchemaLaunch = buildGovernedCodexExecLaunch({
      codexPath: launch.command, codexSha256: launch.cliSha256, cliVersion: launch.cliVersion,
      profile: launch.profile, prompt: 'bounded prompt', mcp: { name: launch.mcpName, url: launch.mcpUrl },
    });
    assert.notEqual(launch.launchDigest, noSchemaLaunch.launchDigest);
    const schemaPath = launch.outputSchemaPath;
    await engine.close();
    assert.throws(() => readFileSync(schemaPath), { code: 'ENOENT' });
  }, { timeoutMs: 2000 });

  const schemaWithUniqueItems = JSON.stringify({
    type: 'object', additionalProperties: false,
    properties: {
      citations: { type: 'array', items: { type: 'string' }, uniqueItems: true },
      nested: { type: 'object', properties: { values: { type: 'array', uniqueItems: true, items: { type: 'string' } } } },
    },
    required: ['citations'], examples: [{ uniqueItems: 'preserve-as-data' }],
  });
  const originalSchema = JSON.parse(schemaWithUniqueItems);
  await withEngine(validEvents(), async (engine, fixtureRoot) => {
    await engine.run(undefined, { schema: schemaWithUniqueItems });
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    const loweredSchema = JSON.parse(observed.schemaText);
    assert.equal(schemaWithUniqueItems, JSON.stringify(originalSchema));
    assert.equal(loweredSchema.type, 'object');
    assert.equal(loweredSchema.additionalProperties, false);
    assert.deepEqual(loweredSchema.required, ['citations']);
    assert.deepEqual(loweredSchema.properties.citations.items, { type: 'string' });
    assert.equal(Object.hasOwn(loweredSchema.properties.citations, 'uniqueItems'), false);
    assert.equal(Object.hasOwn(loweredSchema.properties.nested.properties.values, 'uniqueItems'), false);
    assert.deepEqual(loweredSchema.examples, [{ uniqueItems: 'preserve-as-data' }]);
    assert.equal(engine.launch.outputSchemaDigest, `sha256:${createHash('sha256').update(observed.schemaText).digest('hex')}`);
    assert.equal(validateJsonResponse('{"citations":["a","a"]}', { schema: schemaWithUniqueItems }).isValid, false);
  });
});

test('governed exec keeps system instructions in the supported config channel', async () => {
  await withEngine(validEvents(), async (engine, fixtureRoot) => {
    await engine.run();
    const launch = engine.launch;
    assert.equal(readFileSync(launch.config.model_instructions_file, 'utf8'), 'closed system instruction');
    assert.equal(launch.args.at(-1), 'bounded prompt');
    assert.equal(launch.args.at(-1).includes('closed system instruction'), false);
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    assert.equal(observed.args.at(-1), 'bounded prompt');
  }, { systemPrompt: 'closed system instruction' });
});

test('governed exec rejects unknown categories, session-shaped events, and order violations closed', async () => {
  for (const events of [
    [{ type: 'session_configured' }],
    [{ type: 'thread.started', thread_id: 'thread-1' }, { type: 'unknown.event' }],
    [{ type: 'turn.started' }],
    [
      { type: 'thread.started', thread_id: 'thread-1' }, { type: 'turn.started' },
      { type: 'item.completed', item: { id: 'answer-1', type: 'unexpected_tool', text: 'SECRET' } },
    ],
  ]) {
    await withEngine(events, async engine => {
      await assert.rejects(engine.run(), error => {
        assert.match(error.message, /^GOVERNED_CODEX_EXEC_/);
        assert.equal(error.message.includes('SECRET'), false);
        return true;
      });
    });
  }
});

test('governed exec rejects documented failure events with a closed category diagnostic', async () => {
  for (const event of [
    { type: 'error', message: 'PROVIDER_FAILURE_SECRET' },
    { type: 'turn.failed', error: { message: 'PROVIDER_FAILURE_SECRET' } },
  ]) {
    await withEngine([
      { type: 'thread.started', thread_id: 'thread-1' },
      { type: 'turn.started' },
      event,
    ], async engine => {
      await assert.rejects(engine.run(), error => {
        assert.equal(error.code, 'GOVERNED_CODEX_EXEC_EVENT_CATEGORY');
        const projected = projectGovernedCodexExecFailure(error);
        assert.deepEqual(projected.event, {
          source: 'codex-exec-rejected-failure/v1', category: 'failure', eventType: event.type,
          eventFields: event.type === 'error'
            ? [{ name: 'message', type: 'string', size: 23 }, { name: 'type', type: 'string', size: 5 }]
            : [{ name: 'error', type: 'object' }, { name: 'type', type: 'string', size: 11 }],
        });
        assert.deepEqual(normalizeGovernedCodexExecFailure(error, 'query').providerEngineDiagnostic, projected);
        assert.doesNotMatch(JSON.stringify(projected), /PROVIDER_FAILURE_SECRET/);
        return true;
      });
    });
  }
});

test('governed exec classifies the observed Codex schema rejection without leaking its message', async () => {
  const message = JSON.stringify({
    type: 'error', status: 400,
    error: {
      type: 'invalid_request_error', code: 'invalid_json_schema', param: 'text.format.schema',
      message: "Invalid schema for response_format 'codex_default': In context=('properties', 'citations'), 'uniqueItems' is not permitted.",
    },
  });
  for (const event of [
    { type: 'error', message },
    { type: 'turn.failed', error: { message } },
  ]) {
    await withEngine([
      { type: 'thread.started', thread_id: 'thread-1' },
      { type: 'turn.started' },
      event,
    ], async engine => {
      await assert.rejects(engine.run(), error => {
        const projected = projectGovernedCodexExecFailure(error);
        assert.deepEqual(projected.event.providerError, {
          type: 'invalid_request_error', status: 400, code: 'invalid_json_schema',
          param: 'text.format.schema', schemaKeyword: 'uniqueItems',
        });
        assert.doesNotMatch(JSON.stringify(projected), /uniqueItems not permitted/);
        assert.deepEqual(normalizeGovernedCodexExecFailure(error, 'query').providerEngineDiagnostic, projected);
        return true;
      });
    });
  }
});

test('governed exec binds the last completed agent message and bounded usage', async () => {
  const duplicate = validEvents();
  duplicate.splice(3, 0, { type: 'item.completed', item: { id: 'answer-0', type: 'agent_message', text: 'first' } });
  await withEngine(duplicate, async engine => {
    const result = await engine.run();
    assert.equal(result.answer, '{"ok":true}');
    assert.equal(result.attestation.observed.agentMessageCount, 2);
  });

  const badUsage = validEvents();
  badUsage.at(-1).usage = { input_tokens: 1, output_tokens: -1 };
  await withEngine(badUsage, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_USAGE/));
});

test('governed exec accepts more than the former event quota within framing bounds', async () => {
  const events = [
    { type: 'thread.started', thread_id: 'thread-many-events' },
    { type: 'turn.started' },
    ...Array.from({ length: 254 }, (_, index) => ({
      type: 'item.completed', item: { id: `reason-${index}`, type: 'reasoning', text: 'discard-me' },
    })),
    { type: 'item.completed', item: { id: 'answer-many-events', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 12, cached_input_tokens: 4, output_tokens: 3 } },
  ];
  assert.equal(events.length, 258);
  await withEngine(events, async engine => {
    const result = await engine.run();
    assert.equal(result.answer, '{"ok":true}');
    assert.equal(result.attestation.observed.eventCount, 258);
    assert.equal(result.attestation.observed.completedItemCount, 255);
    assert.equal(result.attestation.observed.agentMessageCount, 1);
    assert.equal(validateGovernedCodexExecAttestation(result.attestation), result.attestation);
  });
});

test('governed exec validates evidence counters relationally and rejects aggregate overflow', async () => {
  await withEngine(validEvents(), async engine => {
    const result = await engine.run();
    const baseObserved = result.attestation.observed;
    const baseEvidence = result.attestation.evidence;
    const assertInvalid = observedPatch => {
      const observed = { ...baseObserved, ...observedPatch };
      const evidence = {
        ...baseEvidence,
        eventCount: observed.eventCount,
        completedItemCount: observed.completedItemCount,
        agentMessageCount: observed.agentMessageCount,
        probeMcpCallCount: observed.probeMcpCallCount,
      };
      assert.throws(() => validateGovernedCodexExecAttestation({
        ...result.attestation, observed, evidence,
      }), TypeError);
    };

    assertInvalid({ eventCount: 5, completedItemCount: 3 });
    assertInvalid({ agentMessageCount: 3 });
    assertInvalid({ probeMcpCallCount: 3 });
  });

  await withEngine(validEvents(), async engine => {
    const result = await engine.run();
    const maximumCompleted = Number.MAX_SAFE_INTEGER - 3;
    const observed = {
      ...result.attestation.observed,
      eventCount: Number.MAX_SAFE_INTEGER,
      completedItemCount: maximumCompleted,
      agentMessageCount: 1,
      probeMcpCallCount: maximumCompleted,
      usedToolItems: [
        { category: 'command_execution', name: null, status: 'completed', count: maximumCompleted },
        { category: 'mcp_tool_call', name: 'mcp__probe__search', status: 'completed', count: maximumCompleted },
      ],
    };
    const evidence = {
      ...result.attestation.evidence,
      eventCount: observed.eventCount,
      completedItemCount: observed.completedItemCount,
      agentMessageCount: observed.agentMessageCount,
      probeMcpCallCount: observed.probeMcpCallCount,
    };
    assert.throws(() => validateGovernedCodexExecAttestation({
      ...result.attestation, observed, evidence,
    }), TypeError);
  }, { profile: nativeProfile(process.cwd()) });
});

test('governed exec fails closed for nonzero exit and incomplete EOF', async () => {
  await withEngine(validEvents(), async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_EXIT/), {}, 'exit-7');
  await withEngine(validEvents().slice(0, 3), async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_INCOMPLETE/));
});

test('governed exec rejects an incomplete tool item at terminal turn completion', async () => {
  const events = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'mcp-1', type: 'mcp_tool_call', name: 'mcp__probe__search', server: '__MCP_SERVER__', arguments: {}, result: null, status: 'in_progress' } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(events, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_INCOMPLETE_ITEM/));
});

test('governed exec requires a tool completion to pair with its start', async () => {
  const events = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.completed', item: { id: 'mcp-1', type: 'mcp_tool_call', name: 'mcp__probe__search', server: '__MCP_SERVER__', arguments: {}, result: {}, status: 'completed' } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(events, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_ITEM_ORDER/));
});

test('governed exec accepts the observed split MCP item shape with exact server binding and evidence', async () => {
  const events = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: {
      arguments: { query: 'dummy' }, error: null, id: 'mcp-1', result: null,
      server: '__MCP_SERVER__', status: 'in_progress', tool: 'mcp__probe__search', type: 'mcp_tool_call',
    } },
    { type: 'item.completed', item: {
      arguments: { query: 'dummy' }, error: null, id: 'mcp-1', result: { content: [{ type: 'text', text: 'dummy result' }] },
      server: '__MCP_SERVER__', status: 'completed', tool: 'mcp__probe__search', type: 'mcp_tool_call',
    } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(events, async (engine, fixtureRoot) => {
    const result = await engine.run();
    assert.deepEqual(result.answer, '{"ok":true}');
    assert.deepEqual(result.evidence.usedToolItems, [{
      category: 'mcp_tool_call', name: 'mcp__probe__search', status: 'completed', count: 1,
    }]);
    assert.equal(result.evidence.probeMcpCallCount, 1);
    assert.equal(result.attestation.observed.probeMcpCallCount, 1);
    assert.equal(result.attestation.observed.usedToolItems[0].name, 'mcp__probe__search');
    assert.equal(JSON.stringify(result).includes('dummy result'), false);
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    assert.equal(observed.codexHome, undefined);
  }, {}, 'mcp-success');

  const combinedEvents = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: {
      arguments: { query: 'dummy' }, id: 'mcp-1', name: 'mcp__probe__search', result: null,
      server: '__MCP_SERVER__', status: 'in_progress', type: 'mcp_tool_call',
    } },
    { type: 'item.completed', item: {
      arguments: { query: 'dummy' }, id: 'mcp-1', name: 'mcp__probe__search', result: { content: [{ type: 'text', text: 'dummy result' }] },
      server: '__MCP_SERVER__', status: 'completed', type: 'mcp_tool_call',
    } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(combinedEvents, async engine => {
    const result = await engine.run();
    assert.deepEqual(result.evidence.usedToolItems, [{
      category: 'mcp_tool_call', name: 'mcp__probe__search', status: 'completed', count: 1,
    }]);
    assert.equal(result.attestation.observed.probeMcpCallCount, 1);
  }, {}, 'mcp-success');
});

test('governed exec rejects split MCP shape mixtures, foreign servers, non-null errors, and unallowed tools', async () => {
  const base = {
    arguments: { query: 'dummy' }, error: null, id: 'mcp-1', result: null,
    server: '__MCP_SERVER__', status: 'in_progress', tool: 'mcp__probe__search', type: 'mcp_tool_call',
  };
  const cases = [
    ['foreign server', { ...base, server: 'probe_foreign' }, 'GOVERNED_CODEX_EXEC_MCP'],
    ['combined foreign server', { ...base, error: undefined, name: 'mcp__probe__search', tool: undefined, server: 'probe_foreign' }, 'GOVERNED_CODEX_EXEC_MCP'],
    ['name/tool mixture', { ...base, name: 'mcp__probe__search' }, 'GOVERNED_CODEX_EXEC_ITEM', 'item_keys'],
    ['non-null error', { ...base, error: 'SECRET_ERROR' }, 'GOVERNED_CODEX_EXEC_ITEM', 'item_error'],
    ['invalid started payload', { ...base, arguments: [] }, 'GOVERNED_CODEX_EXEC_ITEM', 'item_started_payload'],
    ['unallowed tool', { ...base, tool: 'mcp__probe__unknown' }, 'GOVERNED_CODEX_EXEC_TOOL_POLICY'],
  ];
  for (const [label, item, expectedCode, expectedPredicate] of cases) {
    await withEngine([
      { type: 'thread.started', thread_id: 'thread-1' },
      { type: 'turn.started' },
      { type: 'item.started', item },
    ], async engine => {
      await assert.rejects(engine.run(), error => {
        assert.equal(error.code, expectedCode, label);
        if (expectedPredicate) assert.equal(projectGovernedCodexExecFailure(error).event.predicate, expectedPredicate, label);
        assert.doesNotMatch(error.message, /SECRET_ERROR/);
        return true;
      });
    });
  }

  const mismatch = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: base },
    { type: 'item.completed', item: { ...base, error: null, result: {}, status: 'completed', tool: 'mcp__probe__extract' } },
  ];
  await withEngine(mismatch, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_ITEM_ORDER/));
});

test('governed exec accepts a paired failed split MCP completion and normalizes its lifecycle', async () => {
  const failedMcp = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: {
      arguments: { query: 'dummy' }, error: null, id: 'mcp-1', result: null,
      server: '__MCP_SERVER__', status: 'in_progress', tool: 'mcp__probe__search', type: 'mcp_tool_call',
    } },
    { type: 'item.completed', item: {
      arguments: { query: 'dummy' }, error: null, id: 'mcp-1', result: { content: [{ type: 'text', text: 'MCP_RESULT_SECRET' }] },
      server: '__MCP_SERVER__', status: 'failed', tool: 'mcp__probe__search', type: 'mcp_tool_call',
    } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(failedMcp, async engine => {
    const result = await engine.run();
    assert.deepEqual(result.evidence.usedToolItems, [{
      category: 'mcp_tool_call', name: 'mcp__probe__search', status: 'completed', count: 1,
    }]);
    assert.equal(result.evidence.probeMcpCallCount, 1);
    assert.equal(result.attestation.observed.usedToolItems[0].status, 'completed');
    assert.doesNotMatch(JSON.stringify(result), /MCP_RESULT_SECRET/);
  }, {}, 'mcp-success');

  const failedStart = failedMcp.slice(0, 3).map((event, index) => index === 2
    ? { ...event, item: { ...event.item, status: 'failed' } } : event);
  await withEngine(failedStart, async engine => assert.rejects(engine.run(), error => {
    assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
    assert.equal(projectGovernedCodexExecFailure(error).event.predicate, 'item_status');
    assert.equal(projectGovernedCodexExecFailure(error).event.itemStatus, 'failed');
    return true;
  }));

  const missingStartStatus = failedMcp.slice(0, 3).map((event, index) => index === 2
    ? { ...event, item: { ...event.item, status: undefined } } : event);
  await withEngine(missingStartStatus, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_ITEM_STATUS/));

  const missingCompletionStatus = failedMcp.slice(0, 4).map((event, index) => index === 3
    ? { ...event, item: { ...event.item, status: undefined } } : event);
  await withEngine(missingCompletionStatus, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_ITEM_STATUS/));

  const unpaired = failedMcp.filter((event, index) => index !== 2);
  await withEngine(unpaired, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_ITEM_ORDER/));

  const declined = failedMcp.map(event => event.type === 'item.completed' && event.item?.id === 'mcp-1'
    ? { ...event, item: { ...event.item, status: 'declined' } } : event);
  await withEngine(declined, async engine => assert.rejects(engine.run(), error => {
    assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
    assert.equal(projectGovernedCodexExecFailure(error).event.itemStatus, 'declined');
    return true;
  }));

  const fileFailed = failedMcp.slice(0, 2).concat([
    { type: 'item.started', item: { changes: [], id: 'file-1', status: 'in_progress', type: 'file_change' } },
    { type: 'item.completed', item: { changes: [], id: 'file-1', status: 'failed', type: 'file_change' } },
  ]);
  await withEngine(fileFailed, async engine => assert.rejects(engine.run(), error => {
    assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
    assert.equal(projectGovernedCodexExecFailure(error).event.itemStatus, 'failed');
    return true;
  }), { profile: nativeProfile(process.cwd(), 'probe.governed-codex-profile/v3') });

  for (const events of [
    failedMcp.slice(0, 4),
    failedMcp.slice(0, 5),
    failedMcp.slice(0, 5).concat([{ type: 'turn.failed', error: { message: 'hidden' } }]),
    failedMcp.slice(0, 5).concat([{ type: 'error', error: { message: 'hidden' } }]),
  ]) {
    await withEngine(events, async engine => assert.rejects(engine.run(), error => {
      assert.match(error.code, /^GOVERNED_CODEX_EXEC_/);
      assert.doesNotMatch(error.message, /hidden/);
      return true;
    }));
  }
});

test('governed exec accepts only a paired failed command completion and preserves terminal gates', async () => {
  const failedCommand = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { command: 'echo dummy', id: 'cmd-1', status: 'in_progress', type: 'command_execution' } },
    { type: 'item.completed', item: {
      aggregated_output: 'COMMAND_OUTPUT_SECRET', command: 'echo dummy', exit_code: 1,
      id: 'cmd-1', status: 'failed', type: 'command_execution',
    } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(failedCommand, async engine => {
    const result = await engine.run();
    assert.deepEqual(result.evidence.usedToolItems, [{
      category: 'command_execution', name: null, status: 'completed', count: 1,
    }]);
    assert.equal(result.process.exitCode, 0);
    assert.equal(JSON.stringify(result).includes('COMMAND_OUTPUT_SECRET'), false);
    assert.equal(JSON.stringify(result).includes('echo dummy'), false);
  }, { profile: nativeProfile(process.cwd()) });

  for (const [label, events, options, expectedCode] of [
    ['missing final answer', failedCommand.slice(0, 4).concat([{ type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } }]), { profile: nativeProfile(process.cwd()) }, 'GOVERNED_CODEX_EXEC_EVENT_ORDER'],
    ['missing turn completion', failedCommand.slice(0, 5), { profile: nativeProfile(process.cwd()) }, 'GOVERNED_CODEX_EXEC_INCOMPLETE'],
  ]) {
    await withEngine(events, async engine => assert.rejects(engine.run(), error => {
      assert.equal(error.code, expectedCode, label);
      return true;
    }, label), options);
  }
});

test('governed exec accepts a large disposable command transcript without retaining it', async () => {
  const transcript = 'X'.repeat(154334);
  const events = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { command: 'echo transcript', id: 'cmd-1', status: 'in_progress', type: 'command_execution' } },
    { type: 'item.completed', item: {
      aggregated_output: transcript, command: 'echo transcript', exit_code: 0,
      id: 'cmd-1', status: 'completed', type: 'command_execution',
    } },
    { type: 'item.completed', item: { id: 'answer-1', type: 'agent_message', text: '{"ok":true}' } },
    { type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } },
  ];
  await withEngine(events, async engine => {
    const result = await engine.run();
    assert.equal(result.answer, '{"ok":true}');
    assert.equal(JSON.stringify(result).includes(transcript), false);
    assert.equal(JSON.stringify(result).includes('echo transcript'), false);
  }, { profile: nativeProfile(process.cwd()) });
});

test('governed exec rejects a non-string aggregated command transcript with a closed diagnostic', async () => {
  const events = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { command: 'echo transcript', id: 'cmd-1', status: 'in_progress', type: 'command_execution' } },
    { type: 'item.completed', item: {
      aggregated_output: { secret: 'must-not-cross-the-boundary' }, command: 'echo transcript', exit_code: 0,
      id: 'cmd-1', status: 'completed', type: 'command_execution',
    } },
  ];
  await withEngine(events, async engine => {
    await assert.rejects(engine.run(), error => {
      assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
      assert.equal(projectGovernedCodexExecFailure(error).event.predicate, 'item_aggregated_output');
      assert.doesNotMatch(JSON.stringify(projectGovernedCodexExecFailure(error)), /must-not-cross-the-boundary/);
      return true;
    }, 'non-string aggregated_output');
  }, { profile: nativeProfile(process.cwd()) });
});

test('governed exec rejects a command event whose JSONL line exceeds the framing cap', async () => {
  const oversized = 'X'.repeat(1024 * 1024);
  const events = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { aggregated_output: oversized, id: 'cmd-1', type: 'command_execution' } },
  ];
  await withEngine(events, async engine => {
    await assert.rejects(engine.run(), error => {
      assert.equal(error.code, 'GOVERNED_CODEX_EXEC_OUTPUT_OVERFLOW');
      return true;
    });
  });
});

test('governed exec rejects failed or declined statuses outside completed command execution', async () => {
  const commandStartFailed = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { command: 'echo START_SECRET', id: 'cmd-1', status: 'failed', type: 'command_execution' } },
  ];
  await withEngine(commandStartFailed, async engine => {
    await assert.rejects(engine.run(), error => {
      const diagnostic = projectGovernedCodexExecFailure(error);
      assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
      assert.equal(diagnostic.event.predicate, 'item_status');
      assert.equal(diagnostic.event.itemStatus, 'failed');
      const normalized = normalizeGovernedCodexExecFailure(error, 'query');
      assert.equal(normalized.providerEngineDiagnostic.event.itemStatus, 'failed');
      assert.doesNotMatch(JSON.stringify(normalized), /START_SECRET|echo START_SECRET/);
      return true;
    });
  }, { profile: nativeProfile(process.cwd()) });

  const mcpDeclined = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { arguments: {}, id: 'mcp-1', name: 'mcp__probe__search', result: null, server: '__MCP_SERVER__', status: 'in_progress', type: 'mcp_tool_call' } },
    { type: 'item.completed', item: { arguments: {}, id: 'mcp-1', name: 'mcp__probe__search', result: {}, server: '__MCP_SERVER__', status: 'declined', type: 'mcp_tool_call' } },
  ];
  await withEngine(mcpDeclined, async engine => assert.rejects(engine.run(), error => {
    assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
    assert.equal(projectGovernedCodexExecFailure(error).event.itemStatus, 'declined');
    return true;
  }));

  const fileFailed = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { changes: [], id: 'file-1', status: 'in_progress', type: 'file_change' } },
    { type: 'item.completed', item: { changes: [], id: 'file-1', status: 'failed', type: 'file_change' } },
  ];
  await withEngine(fileFailed, async engine => assert.rejects(engine.run(), error => {
    assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
    assert.equal(projectGovernedCodexExecFailure(error).event.itemStatus, 'failed');
    return true;
  }), { profile: nativeProfile(process.cwd(), 'probe.governed-codex-profile/v3') });

  const declined = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { command: 'echo declined', id: 'cmd-1', status: 'in_progress', type: 'command_execution' } },
    { type: 'item.completed', item: { command: 'echo declined', id: 'cmd-1', status: 'declined', type: 'command_execution' } },
  ];
  await withEngine(declined, async engine => assert.rejects(engine.run(), error => {
    const diagnostic = projectGovernedCodexExecFailure(error);
    assert.equal(error.code, 'GOVERNED_CODEX_EXEC_ITEM');
    assert.equal(diagnostic.event.itemStatus, 'declined');
    assert.doesNotMatch(JSON.stringify(diagnostic), /echo declined/);
    return true;
  }), { profile: nativeProfile(process.cwd()) });
});

test('governed exec rejects duplicate item identities and exposes only sanitized startup diagnostics', async () => {
  const duplicate = validEvents();
  duplicate.splice(3, 0, { type: 'item.completed', item: { id: 'reason-1', type: 'reasoning', text: 'secret' } });
  await withEngine(duplicate, async engine => assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_DUPLICATE/));

  const fixtureRoot = fixture(validEvents(), 'exit-7');
  try {
    const original = readFileSync(fixtureRoot.script, 'utf8');
    writeFileSync(fixtureRoot.script, original.replace("process.exitCode = 7;", "process.stderr.write('refresh_token TOP_SECRET /private/tmp/private-value\\n'); process.exitCode = 7;"));
    const currentSha = `sha256:${createHash('sha256').update(readFileSync(fixtureRoot.script)).digest('hex')}`;
    const engine = await createGovernedCodexExecEngine({ agent: agent(fixtureRoot.root), profile: profile(fixtureRoot.root), prompt: 'bounded prompt', codexPath: fixtureRoot.script, codexSha256: currentSha, timeoutMs: 1000 });
    try {
      await assert.rejects(engine.run(), error => {
        assert.equal(error.code, 'GOVERNED_CODEX_EXEC_EXIT');
        assert.equal(error.diagnostic.source, 'codex-exec-stderr/v1');
        assert.equal(error.diagnostic.bytes > 0, true);
        assert.match(error.diagnostic.digest, /^sha256:[0-9a-f]{64}$/);
        assert.equal(Object.hasOwn(error.diagnostic, 'text'), false);
        assert.doesNotMatch(JSON.stringify(error.diagnostic), /TOP_SECRET|private-value/);
        return true;
      });
    } finally { await engine.close(); }
  } finally { rmSync(fixtureRoot.root, { recursive: true, force: true }); }
});

test('governed exec reports closed structural metadata for an unknown item key without its value', async () => {
  const events = validEvents();
  events[3].item['secret key'] = 'TOP_SECRET_ITEM_VALUE';
  await withEngine(events, async engine => {
    await assert.rejects(engine.run(), error => {
      const diagnostic = projectGovernedCodexExecFailure(error);
      assert.deepEqual(Object.keys(diagnostic), ['version', 'code', 'event']);
      assert.equal(diagnostic.code, 'GOVERNED_CODEX_EXEC_ITEM');
      assert.deepEqual(diagnostic.event, {
        source: 'codex-exec-rejected-item/v1', predicate: 'item_keys',
        eventType: 'item.completed', itemType: 'agent_message',
        eventFields: [{ name: 'item', type: 'object' }, { name: 'type', type: 'string', size: 14 }],
        itemFields: [
          { name: '<unsafe>', type: 'string', size: 21 },
          { name: 'id', type: 'string', size: 8 },
          { name: 'text', type: 'string', size: 11 },
          { name: 'type', type: 'string', size: 13 },
        ],
      });
      assert.doesNotMatch(JSON.stringify(diagnostic), /TOP_SECRET_ITEM_VALUE/);
      assert.equal(Object.isFrozen(diagnostic.event), true);
      assert.equal(Object.isFrozen(diagnostic.event.itemFields), true);
      return true;
    });
  });
});

test('exec failure projector drops forged nested event values instead of returning them', () => {
  const diagnostic = projectGovernedCodexExecFailure({
    code: 'GOVERNED_CODEX_EXEC_ITEM',
    event: {
      source: 'codex-exec-rejected-item/v1', predicate: 'item_text',
      eventType: 'item.completed', itemType: 'agent_message',
      eventFields: [{ name: 'item', type: 'object' }, { name: 'type', type: 'string', size: 14 }],
      itemFields: [{ name: 'text', type: 'string', size: 6, value: 'SECRET' }],
    },
  });
  assert.deepEqual(diagnostic, {
    version: 'probe.governed-codex-exec-failure/v1', code: 'GOVERNED_CODEX_EXEC_ITEM',
  });
  assert.doesNotMatch(JSON.stringify(diagnostic), /SECRET/);
});

test('governed exec reports the rejected scalar type and never leaks its value', async () => {
  const events = validEvents();
  events[3].item.text = 987654321;
  await withEngine(events, async engine => {
    await assert.rejects(engine.run(), error => {
      const diagnostic = projectGovernedCodexExecFailure(error);
      assert.equal(diagnostic.code, 'GOVERNED_CODEX_EXEC_ITEM');
      assert.equal(diagnostic.event.predicate, 'item_text');
      assert.deepEqual(diagnostic.event.itemFields.find(field => field.name === 'text'), {
        name: 'text', type: 'number',
      });
      assert.doesNotMatch(JSON.stringify(diagnostic), /987654321/);
      assert.doesNotMatch(JSON.stringify(diagnostic), /TOP_SECRET/);
      return true;
    });
  });
});

test('governed exec reports tool status violations and bounds fields after deterministic sorting', async () => {
  const statusEvents = [
    { type: 'thread.started', thread_id: 'thread-1' },
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'tool-1', type: 'mcp_tool_call', name: 'mcp__probe__search', server: '__MCP_SERVER__', arguments: {}, result: null, status: 'invalid' } },
  ];
  await withEngine(statusEvents, async engine => {
    await assert.rejects(engine.run(), error => {
      const diagnostic = projectGovernedCodexExecFailure(error);
      assert.equal(diagnostic.event.predicate, 'item_status');
      assert.equal(diagnostic.event.eventType, 'item.started');
      assert.equal(diagnostic.event.itemType, 'mcp_tool_call');
      assert.deepEqual(diagnostic.event.itemFields.find(field => field.name === 'status'), {
        name: 'status', type: 'string', size: 7,
      });
      return true;
    });
  });

  const manyFields = validEvents();
  for (let index = 0; index < 40; index += 1) manyFields[3].item[`extra${String(index).padStart(2, '0')}`] = index;
  await withEngine(manyFields, async engine => {
    await assert.rejects(engine.run(), error => {
      const diagnostic = projectGovernedCodexExecFailure(error);
      const fields = diagnostic.event.itemFields;
      assert.equal(diagnostic.event.predicate, 'item_keys');
      assert.equal(fields.length, 32);
      assert.equal(fields[0].name, 'extra00');
      assert.equal(fields.at(-1).name, 'extra31');
      assert.equal(fields.some(field => field.name === 'extra29'), true);
      assert.equal(fields.some(field => field.name === 'extra30'), true);
      assert.equal(fields.some(field => field.name === 'id'), false);
      assert.equal(fields.every(field => !Object.hasOwn(field, 'value')), true);
      return true;
    });
  });
});

test('governed exec retains metadata for an oversized rejected text field', async () => {
  const events = validEvents();
  events[3].item.text = 'x'.repeat(131073);
  await withEngine(events, async engine => {
    await assert.rejects(engine.run(), error => {
      const diagnostic = projectGovernedCodexExecFailure(error);
      assert.equal(diagnostic.event.predicate, 'item_text');
      assert.equal(diagnostic.event.itemFields.find(field => field.name === 'text').size, 131073);
      const normalized = normalizeGovernedCodexExecFailure(error, 'query');
      assert.equal(normalized.providerEngineDiagnostic.event.itemFields.find(field => field.name === 'text').size, 131073);
      assert.doesNotMatch(JSON.stringify(normalized), /xxxxxxxx/);
      return true;
    });
  });
});

test('governed exec cancellation and timeout terminate the process group without retry', async () => {
  await withEngine(validEvents().slice(0, 2), async engine => {
    await assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_TIMEOUT/);
  }, { timeoutMs: 1000, executionTimeoutMs: 30 }, 'hang');

  const controller = new AbortController();
  await withEngine(validEvents(), async engine => {
    controller.abort();
    await assert.rejects(engine.run(), /GOVERNED_CODEX_EXEC_CANCELLED/);
  }, { signal: controller.signal });
});

test('launch rejects a non-absolute or missing Codex identity before starting MCP work', async () => {
  const root = mkdtempSync(join(tmpdir(), 'probe-governed-exec-invalid-'));
  try {
    await assert.rejects(createGovernedCodexExecEngine({
      agent: agent(root), profile: profile(root), prompt: 'x', codexPath: 'codex',
    }), /Invalid Codex executable/);
    await assert.rejects(createGovernedCodexExecEngine({
      agent: agent(root), profile: profile(root), prompt: 'x', codexPath: join(root, 'missing'),
    }), /Invalid Codex executable/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('ProbeAgent selects exec transport only for the explicit governed selector', async () => {
  const fixtureRoot = fixture(validEvents());
  const sha256 = `sha256:${createHash('sha256').update(readFileSync(fixtureRoot.script)).digest('hex')}`;
  try {
    const governed = new ProbeAgent({
      provider: 'codex', path: fixtureRoot.root, cwd: fixtureRoot.root,
      allowedTools: ['search', 'extract', 'listFiles'], governedCodexProfile: profile(fixtureRoot.root),
      governedCodexTransport: 'exec-jsonl-default-auth-v1', codexBin: fixtureRoot.script, codexSha256: sha256,
      disableMermaidValidation: true,
    });
    const schema = '{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}';
    const preview = await governed.previewGovernedAnswerDispatch('return ok', { schema });
    const result = await governed.answerGoverned('return ok', { schema, invocationDigest: `sha256:${'a'.repeat(64)}` });
    assert.deepEqual(result.data, { ok: true });
    assert.equal(result.runtimeAttestation.version, 'probe.governed-codex-exec-attestation/v1');
    assert.deepEqual(preview, result.runtimeAttestation.dispatch);
    const observed = JSON.parse(readFileSync(fixtureRoot.observed, 'utf8'));
    const schemaFlag = observed.args.indexOf('--output-schema');
    assert.equal(schemaFlag >= 0, true);
    assert.equal(observed.args.at(-2), observed.schemaPath);
    assert.equal(observed.args.at(-1).startsWith('return ok'), true);
    assert.equal(observed.schemaText, schema);
    assert.equal(observed.schemaMode, 0o600);
    assert.notDeepEqual(
      previewGovernedCodexExecDispatch('same user prompt', 'system one'),
      previewGovernedCodexExecDispatch('same user prompt', 'system two'),
    );

    const tampered = {
      ...result.runtimeAttestation,
      observed: { ...result.runtimeAttestation.observed, finalDigest: `sha256:${'0'.repeat(64)}` },
    };
    governed.getEngine = async () => ({
      query: async function* () {
        yield { type: 'text', content: '{"ok":true}' };
        yield { type: 'metadata', data: { attestation: tampered } };
      },
      close: async () => {},
    });
    await assert.rejects(
      governed.answerGoverned('return ok', { schema, invocationDigest: `sha256:${'a'.repeat(64)}` }),
      error => error.name === 'GovernedAnswerFailure' && error.answerFailureStage === 'native_event_grammar',
    );
  } finally {
    rmSync(fixtureRoot.root, { recursive: true, force: true });
  }
});

test('ProbeAgent ordinary answer forwards only serialized JSON schemas to exec', async () => {
  const schema = '{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}';
  const schemaFixture = fixture(validEvents());
  const builtinFixture = fixture(validEvents());
  const makeAgent = fixtureRoot => new ProbeAgent({
    provider: 'codex', path: fixtureRoot.root, cwd: fixtureRoot.root,
    allowedTools: ['search', 'extract', 'listFiles'], governedCodexProfile: profile(fixtureRoot.root),
    governedCodexTransport: 'exec-jsonl-default-auth-v1', codexBin: fixtureRoot.script,
    codexSha256: `sha256:${createHash('sha256').update(readFileSync(fixtureRoot.script)).digest('hex')}`,
    disableMermaidValidation: true,
  });
  try {
    const schemaAgent = makeAgent(schemaFixture);
    assert.equal(await schemaAgent.answer('return ok', [], { schema }), '{"ok":true}');
    const schemaObserved = JSON.parse(readFileSync(schemaFixture.observed, 'utf8'));
    const schemaFlag = schemaObserved.args.indexOf('--output-schema');
    assert.equal(schemaFlag >= 0, true);
    assert.equal(schemaObserved.args[schemaFlag + 1], schemaObserved.schemaPath);
    assert.equal(schemaObserved.args.at(-1), 'return ok');
    assert.equal(schemaObserved.schemaText, schema);
    assert.equal(schemaObserved.schemaMode, 0o600);

    const builtinAgent = makeAgent(builtinFixture);
    assert.equal(await builtinAgent.answer('return ok', [], { schema: 'renderer-name' }), '{"ok":true}');
    const builtinObserved = JSON.parse(readFileSync(builtinFixture.observed, 'utf8'));
    assert.equal(builtinObserved.schemaPath, null);
    assert.equal(builtinObserved.args.includes('--output-schema'), false);
    assert.equal(builtinObserved.args.at(-1), 'return ok');
  } finally {
    rmSync(schemaFixture.root, { recursive: true, force: true });
    rmSync(builtinFixture.root, { recursive: true, force: true });
  }
});

test('ProbeAgent preserves only the closed exec failure diagnostic through answerGoverned', async () => {
  const fixtureRoot = fixture(validEvents());
  const sha256 = `sha256:${createHash('sha256').update(readFileSync(fixtureRoot.script)).digest('hex')}`;
  const schema = '{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}';
  try {
    const governed = new ProbeAgent({
      provider: 'codex', path: fixtureRoot.root, cwd: fixtureRoot.root,
      allowedTools: ['search', 'extract', 'listFiles'], governedCodexProfile: profile(fixtureRoot.root),
      governedCodexTransport: 'exec-jsonl-default-auth-v1', codexBin: fixtureRoot.script, codexSha256: sha256,
      disableMermaidValidation: true,
    });
    const makeFailure = (code, stderr) => Object.assign(new Error('TOP_SECRET_ERROR'), { code, diagnostic: stderr });
    const revoked = {
      source: 'codex-exec-stderr/v1', bytes: 41, digest: `sha256:${'b'.repeat(64)}`,
      safeMessage: 'access_token_refresh_revoked', text: 'REFRESH_TOKEN_SECRET',
    };
    governed.getEngine = async () => { throw makeFailure('GOVERNED_CODEX_EXEC_EXIT', revoked); };
    await assert.rejects(governed.answerGoverned('return ok', { schema }), error => {
      assert.equal(error.name, 'GovernedAnswerFailure');
      assert.equal(error.answerFailureStage, 'provider_engine');
      assert.equal(error.providerEngineFailureBoundary, 'acquire');
      assert.deepEqual(Object.keys(error), ['answerFailureStage', 'providerEngineFailureBoundary', 'providerEngineDiagnostic']);
      assert.deepEqual(error.providerEngineDiagnostic, {
        version: 'probe.governed-codex-exec-failure/v1', code: 'GOVERNED_CODEX_EXEC_EXIT',
        stderr: { source: 'codex-exec-stderr/v1', bytes: 41, digest: `sha256:${'b'.repeat(64)}`, safeMessage: 'access_token_refresh_revoked' },
      });
      assert.doesNotMatch(JSON.stringify(error), /TOP_SECRET|REFRESH_TOKEN_SECRET/);
      return true;
    });

    governed.getEngine = async () => {
      throw makeFailure('GOVERNED_CODEX_EXEC_EXIT', {
        source: 'codex-exec-stderr/v1', bytes: 17, digest: `sha256:${'c'.repeat(64)}`, text: 'ARBITRARY_SECRET',
      });
    };
    await assert.rejects(governed.answerGoverned('return ok', { schema }), error => {
      assert.equal(error.providerEngineFailureBoundary, 'acquire');
      assert.deepEqual(error.providerEngineDiagnostic, {
        version: 'probe.governed-codex-exec-failure/v1', code: 'GOVERNED_CODEX_EXEC_EXIT',
        stderr: { source: 'codex-exec-stderr/v1', bytes: 17, digest: `sha256:${'c'.repeat(64)}` },
      });
      assert.doesNotMatch(JSON.stringify(error), /ARBITRARY_SECRET/);
      return true;
    });

    governed.getEngine = async () => { throw makeFailure('UNRECOGNIZED_EXEC_CODE', revoked); };
    await assert.rejects(governed.answerGoverned('return ok', { schema }), error => {
      assert.equal(error.providerEngineFailureBoundary, 'acquire');
      assert.equal(Object.hasOwn(error, 'providerEngineDiagnostic'), false);
      return true;
    });
  } finally {
    rmSync(fixtureRoot.root, { recursive: true, force: true });
  }
});
