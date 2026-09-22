import { createHash } from 'node:crypto';
import { realpathSync, statSync } from 'node:fs';
import { isAbsolute, normalize } from 'node:path';

const PROFILE_KEYS = ['version', 'profileId', 'engine', 'model', 'reasoningEffort', 'sandbox', 'approvalPolicy', 'cwd', 'probeTools', 'fallback', 'retries'];
const PROFILE_V2_KEYS = ['version', 'profileId', 'engine', 'model', 'reasoningEffort', 'sandbox', 'approvalPolicy', 'cwd', 'probeMcpTools', 'codexNativeTools', 'fallback', 'retries'];
const PROFILE_VALUES = {
  version: 'probe.governed-codex-profile/v1', profileId: 'luna-xhigh-readonly-v1', engine: 'codex',
  model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never',
  fallback: false, retries: 0,
};
const PROFILE_V2_VALUES = { ...PROFILE_VALUES, version: 'probe.governed-codex-profile/v2', profileId: 'luna-xhigh-readonly-native-exec-v1' };
// This profile admits the pinned Codex `exec` capability inside the attested read-only sandbox.
// It does not claim that commands supplied to `exec` are semantically safe.
const PROBE_TOOLS = ['search', 'extract', 'listFiles'];
const CODEX_NATIVE_TOOLS = ['exec'];
const ENABLED_TOOLS = ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'];
const SESSION_MSG_KEYS = ['type', 'session_id', 'thread_id', 'model', 'model_provider_id', 'approval_policy',
  'approvals_reviewer', 'permission_profile', 'reasoning_effort', 'rollout_path', 'cwd'];
const SESSION_SERVICE_TIERS = ['default', 'priority', 'flex'];
const FEATURE_NAMES = [
  'shell_tool', 'multi_agent', 'multi_agent_v2', 'enable_fanout', 'apps', 'enable_mcp_apps',
  'tool_suggest', 'plugins', 'in_app_browser', 'browser_use', 'browser_use_full_cdp_access',
  'browser_use_external', 'computer_use', 'remote_plugin', 'plugin_sharing', 'image_generation',
  'skill_mcp_dependency_install', 'hooks', 'request_permissions_tool', 'standalone_web_search',
];
const SAFE_ID = /^[A-Za-z0-9._:-]{1,128}$/;
const ROLLOUT = /\/sessions\/(\d{4})\/(0[1-9]|1[0-2])\/(0[1-9]|[12]\d|3[01])\/rollout-(\d{4})-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])T([01]\d|2[0-3])-([0-5]\d)-([0-5]\d)-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.jsonl$/;

function invalid(label) { throw new TypeError(`Invalid ${label}`); }
function enumerableKeys(value) {
  return Reflect.ownKeys(value).filter((key) => Object.prototype.propertyIsEnumerable.call(value, key));
}
function exactObject(value, keys, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) invalid(label);
  const proto = Object.getPrototypeOf(value);
  if (proto !== Object.prototype && proto !== null) invalid(label);
  const actual = enumerableKeys(value);
  if (actual.some((key) => typeof key !== 'string') || actual.length !== keys.length || keys.some((key) => !actual.includes(key))) invalid(label);
  for (const key of keys) if (!Object.prototype.hasOwnProperty.call(Object.getOwnPropertyDescriptor(value, key), 'value')) invalid(label);
  return value;
}
function exactArray(value, length, label) {
  if (!Array.isArray(value) || value.length !== length) invalid(label);
  const keys = enumerableKeys(value);
  if (keys.length !== length || keys.some((key, index) => key !== String(index))) invalid(label);
  return value;
}
function deepFreeze(value) {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const key of Reflect.ownKeys(value)) deepFreeze(value[key]);
    Object.freeze(value);
  }
  return value;
}
function canonicalJson(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number' && Number.isFinite(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  invalid('canonical JSON value');
}
function digest(value) { return createHash('sha256').update(canonicalJson(value), 'utf8').digest('hex'); }
function canonicalCwd(value) {
  if (typeof value !== 'string' || !isAbsolute(value) || value.includes('\0')) invalid('cwd');
  try {
    const resolved = realpathSync(value);
    if (!statSync(resolved).isDirectory()) invalid('cwd');
    return resolved;
  } catch { invalid('cwd'); }
}
function requireValue(actual, expected, label) { if (!Object.is(actual, expected)) invalid(label); }

export function validateGovernedCodexProfile(input) {
  const version = input && Object.getOwnPropertyDescriptor(input, 'version')?.value;
  const v2 = version === PROFILE_V2_VALUES.version, values = v2 ? PROFILE_V2_VALUES : PROFILE_VALUES;
  const profile = exactObject(input, v2 ? PROFILE_V2_KEYS : PROFILE_KEYS, 'profile');
  for (const [key, value] of Object.entries(values)) requireValue(profile[key], value, `profile.${key}`);
  const probeKey = v2 ? 'probeMcpTools' : 'probeTools';
  exactArray(profile[probeKey], PROBE_TOOLS.length, `profile.${probeKey}`);
  PROBE_TOOLS.forEach((tool, index) => requireValue(profile[probeKey][index], tool, `profile.${probeKey}[${index}]`));
  if (v2) {
    exactArray(profile.codexNativeTools, CODEX_NATIVE_TOOLS.length, 'profile.codexNativeTools');
    CODEX_NATIVE_TOOLS.forEach((tool, index) => requireValue(profile.codexNativeTools[index], tool, `profile.codexNativeTools[${index}]`));
    if (profile.probeMcpTools.some((tool) => profile.codexNativeTools.includes(tool))) invalid('profile capability overlap');
  }
  return deepFreeze({
    version: values.version, profileId: values.profileId, engine: values.engine,
    model: values.model, reasoningEffort: values.reasoningEffort, sandbox: values.sandbox,
    approvalPolicy: values.approvalPolicy, cwd: canonicalCwd(profile.cwd),
    ...(v2 ? { probeMcpTools: [...PROBE_TOOLS], codexNativeTools: [...CODEX_NATIVE_TOOLS] } : { probeTools: [...PROBE_TOOLS] }),
    fallback: false, retries: 0,
  });
}

function validateMcp(input) {
  const mcp = exactObject(input, ['name', 'url'], 'mcp');
  if (typeof mcp.name !== 'string' || !/^probe_[0-9a-f]{16}$/.test(mcp.name) || typeof mcp.url !== 'string') invalid('mcp');
  const raw = /^http:\/\/127\.0\.0\.1:([0-9]{1,5})\/mcp$/.exec(mcp.url);
  let parsed;
  try { parsed = new URL(mcp.url); } catch { invalid('mcp.url'); }
  const port = raw && Number(raw[1]);
  if (!raw || String(port) !== raw[1] || port < 1 || port > 65535 || parsed.protocol !== 'http:' || parsed.hostname !== '127.0.0.1' || (parsed.port !== raw[1] && !(port === 80 && parsed.port === '')) || parsed.pathname !== '/mcp' || parsed.username || parsed.password || parsed.search || parsed.hash) invalid('mcp.url');
  return mcp;
}

export function buildGovernedCodexInitialToolArgs(input) {
  exactObject(input, ['profile', 'prompt', 'mcp'], 'builder input');
  const profile = validateGovernedCodexProfile(input.profile);
  if (typeof input.prompt !== 'string' || Buffer.byteLength(input.prompt, 'utf8') < 1 || Buffer.byteLength(input.prompt, 'utf8') > 131072) invalid('prompt');
  const mcp = validateMcp(input.mcp);
  const features = Object.fromEntries(FEATURE_NAMES.map((name) => [name, false]));
  const tools = Object.fromEntries(ENABLED_TOOLS.map((name) => [name, { approval_mode: 'approve' }]));
  const server = { url: mcp.url, default_tools_approval_mode: 'prompt', enabled_tools: [...ENABLED_TOOLS], tools };
  return deepFreeze({
    prompt: input.prompt, model: profile.model,
    config: { model_reasoning_effort: profile.reasoningEffort, web_search: 'disabled', features, skills: { include_instructions: false }, mcp_servers: { [mcp.name]: server } },
    cwd: profile.cwd, sandbox: profile.sandbox, 'approval-policy': profile.approvalPolicy,
  });
}

function validatePermission(input) {
  const permission = exactObject(input, ['type', 'file_system', 'network'], 'permission_profile');
  requireValue(permission.type, 'managed', 'permission_profile.type');
  requireValue(permission.network, 'restricted', 'permission_profile.network');
  const fileSystem = exactObject(permission.file_system, ['type', 'entries'], 'file_system');
  requireValue(fileSystem.type, 'restricted', 'file_system.type');
  exactArray(fileSystem.entries, 1, 'file_system.entries');
  const entry = exactObject(fileSystem.entries[0], ['access', 'path'], 'file_system entry');
  requireValue(entry.access, 'read', 'file_system entry access');
  const path = exactObject(entry.path, ['type', 'value'], 'permission path');
  requireValue(path.type, 'special', 'permission path type');
  const value = exactObject(path.value, ['kind'], 'permission path value');
  requireValue(value.kind, 'root', 'permission path kind');
  return { type: 'managed', file_system: { type: 'restricted', entries: [{ access: 'read', path: { type: 'special', value: { kind: 'root' } } }] }, network: 'restricted' };
}

function validateRollout(value) {
  if (typeof value !== 'string' || Buffer.byteLength(value, 'utf8') > 4096 || value.includes('\0') || !isAbsolute(value) || normalize(value) !== value) invalid('rollout_path');
  const match = ROLLOUT.exec(value);
  if (!match || match.index + match[0].length !== value.length || match[1] !== match[4] || match[2] !== match[5] || match[3] !== match[6]) invalid('rollout_path');
}

export function attestGovernedCodexSession(input) {
  exactObject(input, ['profile', 'events'], 'attester input');
  const profile = validateGovernedCodexProfile(input.profile);
  const v2 = profile.version === PROFILE_V2_VALUES.version;
  if (v2) {
    exactArray(input.events, 2, 'events');
  } else exactArray(input.events, 1, 'events');
  const event = exactObject(input.events[0], ['jsonrpc', 'method', 'params'], 'event');
  requireValue(event.jsonrpc, '2.0', 'event.jsonrpc'); requireValue(event.method, 'codex/event', 'event.method');
  const params = exactObject(event.params, ['_meta', 'id', 'msg'], 'event.params'); requireValue(params.id, '', 'event.params.id');
  const meta = exactObject(params._meta, ['requestId', 'threadId'], 'event._meta'); requireValue(meta.requestId, 2, 'requestId');
  const hasServiceTier = Object.prototype.propertyIsEnumerable.call(params.msg, 'service_tier');
  const msg = exactObject(params.msg, hasServiceTier ? [...SESSION_MSG_KEYS, 'service_tier'] : SESSION_MSG_KEYS, 'event.msg');
  if (hasServiceTier && !SESSION_SERVICE_TIERS.includes(msg.service_tier)) invalid('event.msg');
  for (const id of [meta.threadId, msg.session_id, msg.thread_id]) if (typeof id !== 'string' || !SAFE_ID.test(id)) invalid('session identity');
  if (meta.threadId !== msg.session_id || msg.session_id !== msg.thread_id) invalid('session identity');
  requireValue(msg.type, 'session_configured', 'msg.type'); requireValue(msg.model, profile.model, 'msg.model');
  requireValue(msg.model_provider_id, 'openai', 'msg.model_provider_id'); requireValue(msg.approval_policy, profile.approvalPolicy, 'msg.approval_policy');
  requireValue(msg.approvals_reviewer, 'user', 'msg.approvals_reviewer'); requireValue(msg.reasoning_effort, profile.reasoningEffort, 'msg.reasoning_effort');
  validateRollout(msg.rollout_path);
  if (canonicalCwd(msg.cwd) !== profile.cwd) invalid('msg.cwd');
  const permission = validatePermission(msg.permission_profile);
  const cwdDigest = digest(profile.cwd);
  if (v2) {
    const nativeTools = exactObject(input.events[1], ['total', 'tools'], 'native tool evidence');
    if (!Number.isSafeInteger(nativeTools.total) || nativeTools.total < 0 || nativeTools.total > 256) invalid('native tool total');
    exactArray(nativeTools.tools, nativeTools.total === 0 ? 0 : 1, 'native tool aggregates');
    if (nativeTools.total > 0) {
      const aggregate = exactObject(nativeTools.tools[0], ['name', 'status', 'count'], 'native tool aggregate');
      if (!profile.codexNativeTools.includes(aggregate.name)) invalid('undeclared native tool evidence');
      requireValue(aggregate.status, 'completed', 'native tool status');
      if (!Number.isSafeInteger(aggregate.count) || aggregate.count !== nativeTools.total) invalid('native tool count');
    }
    return deepFreeze({
      version: 'probe.governed-codex-attestation/v3', profileId: profile.profileId,
      requested: { profileDigest: digest(profile), cwdDigest, probeMcpToolsDigest: digest(profile.probeMcpTools),
        codexNativeToolsDigest: digest(profile.codexNativeTools), probeMcpTools: [...profile.probeMcpTools],
        codexNativeTools: [...profile.codexNativeTools], model: profile.model, reasoningEffort: profile.reasoningEffort,
        sandbox: profile.sandbox, approvalPolicy: profile.approvalPolicy },
      observed: { source: 'session_configured+raw_response_item', model: msg.model, modelProviderId: msg.model_provider_id,
        reasoningEffort: msg.reasoning_effort, approvalPolicy: msg.approval_policy, cwdDigest,
        permissionProfileDigest: digest(permission), filesystem: 'restricted-read-root', network: permission.network,
        nativeTools: { total: nativeTools.total, tools: nativeTools.tools.map((item) => ({ ...item })) } },
      correlation: { requestId: 2, eventCount: nativeTools.total + 1 }, usage: { status: 'unavailable' },
    });
  }
  return deepFreeze({
    version: 'probe.governed-codex-attestation/v1', profileId: profile.profileId,
    requested: { profileDigest: digest(profile), cwdDigest, probeToolsDigest: digest(profile.probeTools), model: profile.model, reasoningEffort: profile.reasoningEffort, sandbox: profile.sandbox, approvalPolicy: profile.approvalPolicy },
    observed: { source: 'session_configured', model: msg.model, modelProviderId: msg.model_provider_id, reasoningEffort: msg.reasoning_effort, approvalPolicy: msg.approval_policy, cwdDigest, permissionProfileDigest: digest(permission), filesystem: 'restricted-read-root', network: permission.network },
    correlation: { requestId: 2, eventCount: 1 }, usage: { status: 'unavailable' },
  });
}
