/**
 * Capture-governed Codex MCP transport.
 *
 * This module intentionally implements only the one-turn Codex 0.144.1
 * protocol used by the governed Probe adapter.  The wire contract is kept
 * explicit here so an unknown message cannot be silently treated as noise.
 */

import { spawn } from 'child_process';
import { createHash, randomBytes } from 'crypto';
import { createInterface } from 'readline';
import {
  existsSync,
  lstatSync,
  readdirSync,
  realpathSync,
  readFileSync,
  statSync
} from 'fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'path';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { Session } from '../shared/Session.js';
import { isAuthenticProbeAgent } from '../governance-marker.js';

export const CODEX_MODEL = 'gpt-5.6-luna';
export const CODEX_REASONING_EFFORT = 'xhigh';
export const CODEX_SANDBOX = 'read-only';
export const CODEX_EXTENSION_SANDBOX = 'seatbelt';
export const CODEX_APPROVAL_POLICY = 'never';
export const CODEX_DEFAULT_TIMEOUT = 600000;
export const CODEX_TIMEOUT_MIN = 1;
export const CODEX_TIMEOUT_MAX = 1200000;
export const CODEX_PINNED_SERVER_VERSION = '0.144.1';
export const CODEX_PINNED_EXECUTABLE_PATH = '/opt/homebrew/Caskroom/codex/0.144.1/bin/codex';
export const CODEX_PINNED_EXECUTABLE_SHA256 = '29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a';
export const CODEX_STDERR_MAX_BYTES = 65536;
export const CODEX_QUIET_WINDOW_MS = 1500;
export const CODEX_CLEANUP_TIMEOUT_MS = 1000;
export const CODEX_MAX_EVENT_COUNT = 256;
export const CODEX_MAX_SERIALIZED_BYTES = 1048576;
export const CODEX_MAX_INCOMING_BYTES = CODEX_MAX_SERIALIZED_BYTES;

const INITIALIZE_PROTOCOL_VERSION = '2024-11-05';
const INITIALIZE_CLIENT_INFO = Object.freeze({ name: 'protocol-capture-r4-tool', version: '1.0.0' });
const PROBE_TOOL_PREFIX = 'mcp__probe__';
const PROBE_TOOLS = Object.freeze(['search', 'extract', 'listFiles']);
const PROBE_TOOL_SET = new Set(PROBE_TOOLS);
const EVENT_TYPES = new Set([
  'agent_message',
  'agent_message_content_delta',
  'item_completed',
  'item_started',
  'mcp_startup_complete',
  'mcp_startup_update',
  'mcp_tool_call_begin',
  'mcp_tool_call_end',
  'raw_response_item',
  'session_configured',
  'task_complete',
  'task_started',
  'token_count',
  'user_message'
]);
const FEATURE_OVERRIDES = Object.freeze({
  shell_tool: false,
  multi_agent: false,
  multi_agent_v2: false,
  enable_fanout: false,
  apps: false,
  enable_mcp_apps: false,
  tool_suggest: false,
  plugins: false,
  in_app_browser: false,
  browser_use: false,
  browser_use_full_cdp_access: false,
  browser_use_external: false,
  computer_use: false,
  remote_plugin: false,
  plugin_sharing: false,
  image_generation: false,
  skill_mcp_dependency_install: false,
  hooks: false,
  request_permissions_tool: false,
  standalone_web_search: false
});
const ALLOWED_ENVIRONMENT = Object.freeze(['PATH', 'TMPDIR', 'LANG', 'LC_ALL', 'USER', 'LOGNAME', 'SHELL', 'TERM']);

const MAX_STRING_LENGTH = 131072;
const MAX_ID_LENGTH = 512;
const MAX_EVENT_TYPE_LENGTH = 64;
const MAX_ACTIVITY = 256;
const MAX_ITEMS = 128;
const MAX_MCP_CALLS = 32;
const MAX_BRIDGE_CALLS = 64;
const MAX_TOKEN_COUNTS = 64;

const EXPECTED_PROBE_TOOL_LIST_RESULT = Object.freeze({
  tools: [
    {
      name: 'mcp__probe__search',
      description: 'Search for code patterns using semantic search',
      inputSchema: {
        type: 'object',
        properties: {
          query: { type: 'string', description: 'Search query' },
          path: { type: 'string', description: 'Directory to search', default: '.' },
          maxResults: { type: 'integer', default: 10 }
        },
        required: ['query']
      }
    },
    {
      name: 'mcp__probe__extract',
      description: 'Extract code from specific file location',
      inputSchema: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'File path with optional line number' }
        },
        required: ['path']
      }
    },
    {
      name: 'mcp__probe__listFiles',
      description: 'List files in a directory',
      inputSchema: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'Directory path' },
          pattern: { type: 'string', description: 'File pattern' }
        },
        required: ['path']
      }
    }
  ]
});

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value, keys) {
  if (!isObject(value)) return false;
  const expected = new Set(keys);
  const actual = Object.keys(value);
  return actual.length === expected.size && actual.every(key => expected.has(key));
}

function requiredKeys(value, keys) {
  if (!isObject(value)) return false;
  return keys.every(key => Object.prototype.hasOwnProperty.call(value, key));
}

function cloneJson(value, field, maxBytes = CODEX_MAX_SERIALIZED_BYTES) {
  let serialized;
  try {
    serialized = JSON.stringify(value);
  } catch {
    throw new Error(`Codex ${field} is not JSON-serializable`);
  }
  if (typeof serialized !== 'string' || Buffer.byteLength(serialized, 'utf8') > maxBytes) {
    throw new Error(`Codex ${field} exceeds the serialized-byte bound`);
  }
  return JSON.parse(serialized);
}

function requireString(value, field, maxLength = MAX_STRING_LENGTH) {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) {
    throw new Error(`Codex ${field} is unavailable or invalid`);
  }
  return value;
}

function requireNumber(value, field, { integer = false, min = Number.MIN_SAFE_INTEGER } = {}) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || (integer && !Number.isInteger(value))) {
    throw new Error(`Codex ${field} is unavailable or invalid`);
  }
  return value;
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (isObject(value)) {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function sameJson(left, right) {
  return canonicalJson(left) === canonicalJson(right);
}

export function canonicalizeCodexCwd(cwd) {
  const value = requireString(cwd, 'cwd', 4096);
  if (!isAbsolute(value)) throw new Error('Codex cwd must be absolute');
  try {
    return realpathSync(resolve(value));
  } catch {
    throw new Error('Codex cwd is unavailable or cannot be canonicalized');
  }
}

export function validateCodexBindings({
  model,
  thinkingEffort,
  cwd,
  sandbox = CODEX_SANDBOX,
  approvalPolicy = CODEX_APPROVAL_POLICY,
  codexMcpTimeout = CODEX_DEFAULT_TIMEOUT
} = {}) {
  if (model !== CODEX_MODEL) throw new Error(`Invalid Codex model: ${String(model)}`);
  if (thinkingEffort !== CODEX_REASONING_EFFORT) throw new Error(`Invalid Codex reasoning effort: ${String(thinkingEffort)}`);
  if (sandbox !== CODEX_SANDBOX) throw new Error(`Invalid Codex sandbox: ${String(sandbox)}`);
  if (approvalPolicy !== CODEX_APPROVAL_POLICY) throw new Error(`Invalid Codex approval policy: ${String(approvalPolicy)}`);
  if (!Number.isInteger(codexMcpTimeout) || codexMcpTimeout < CODEX_TIMEOUT_MIN || codexMcpTimeout > CODEX_TIMEOUT_MAX) {
    throw new Error(`Invalid Codex MCP timeout: ${String(codexMcpTimeout)}`);
  }
  return {
    model,
    thinkingEffort,
    cwd: canonicalizeCodexCwd(cwd),
    sandbox,
    approvalPolicy,
    codexMcpTimeout
  };
}

export function buildCodexInitialToolArgs({
  prompt,
  model,
  thinkingEffort,
  cwd,
  sandbox = CODEX_SANDBOX,
  approvalPolicy = CODEX_APPROVAL_POLICY,
  mcpServerName,
  mcpServerUrl
} = {}) {
  const bindings = validateCodexBindings({ model, thinkingEffort, cwd, sandbox, approvalPolicy });
  requireString(prompt, 'prompt');
  if (typeof mcpServerName !== 'string' || mcpServerName.length === 0 ||
      typeof mcpServerUrl !== 'string' || mcpServerUrl.length === 0) {
    throw new Error('Codex Probe MCP server binding is unavailable or invalid');
  }
  const config = {
    model_reasoning_effort: bindings.thinkingEffort,
    web_search: 'disabled',
    features: { ...FEATURE_OVERRIDES },
    skills: { include_instructions: false },
    mcp_servers: {
      [mcpServerName]: {
        url: mcpServerUrl,
        default_tools_approval_mode: 'prompt',
        enabled_tools: PROBE_TOOLS.map(tool => `${PROBE_TOOL_PREFIX}${tool}`),
        tools: Object.fromEntries(PROBE_TOOLS.map(tool => [
          `${PROBE_TOOL_PREFIX}${tool}`, { approval_mode: 'approve' }
        ]))
      }
    }
  };
  return {
    prompt,
    model: bindings.model,
    config,
    cwd: bindings.cwd,
    sandbox: bindings.sandbox,
    'approval-policy': bindings.approvalPolicy
  };
}

export function buildCodexRequestedMetadata(bindings) {
  const validated = validateCodexBindings(bindings);
  return {
    model: validated.model,
    reasoning_effort: validated.thinkingEffort,
    cwd: validated.cwd,
    sandbox: validated.sandbox,
    approval_policy: validated.approvalPolicy,
    timeout_ms: validated.codexMcpTimeout
  };
}

function verifyExecutable({ executablePath, expectedExecutablePath, expectedExecutableSha256 }) {
  if ((executablePath !== undefined && executablePath !== CODEX_PINNED_EXECUTABLE_PATH) ||
      (expectedExecutablePath !== undefined && expectedExecutablePath !== CODEX_PINNED_EXECUTABLE_PATH) ||
      (expectedExecutableSha256 !== undefined && expectedExecutableSha256 !== CODEX_PINNED_EXECUTABLE_SHA256)) {
    throw new Error('Codex executable path and SHA-256 are fixed to the captured binary');
  }
  const expectedSha256 = CODEX_PINNED_EXECUTABLE_SHA256;
  let canonicalExecutable;
  try {
    canonicalExecutable = realpathSync(resolve(executablePath || CODEX_PINNED_EXECUTABLE_PATH));
  } catch {
    throw new Error('Codex executable path cannot be canonicalized');
  }
  if (canonicalExecutable !== CODEX_PINNED_EXECUTABLE_PATH) throw new Error('Codex executable path pin does not match');
  const sha256 = createHash('sha256').update(readFileSync(canonicalExecutable)).digest('hex');
  if (sha256 !== expectedSha256) throw new Error('Codex executable SHA-256 does not match');
  return { path: canonicalExecutable, sha256 };
}

function isDescendantPath(parent, candidate) {
  const child = relative(parent, candidate);
  return child !== '' && child !== '..' && !child.startsWith(`..${sep}`) && !isAbsolute(child);
}

function validateGovernedAgent(agent) {
  if (!isAuthenticProbeAgent(agent)) throw new Error('Codex Probe governance requires the real top-level ProbeAgent');
  for (const field of [
    'enableDelegate', 'enableExecutePlan', 'allowEdit', 'enableBash', 'enableSkills',
    'enableTasks', 'enableMcp'
  ]) {
    if (agent[field] !== false) throw new Error(`Codex Probe governance violation: ${field}`);
  }
  if (agent.searchDelegate !== false) throw new Error('Codex Probe governance violation: searchDelegate');
  if (agent.fallbackConfig !== false) throw new Error('Codex Probe governance violation: fallback is enabled');
  if (agent.disableMermaidValidation !== true || agent.disableJsonValidation !== true) {
    throw new Error('Codex Probe governance requires validation bypasses to be explicit');
  }
  for (const field of ['completionPrompt', 'mcpConfig', 'mcpConfigPath', 'mcpServers', 'mcpBridge', 'maxIterations']) {
    if (agent[field] !== null && agent[field] !== undefined) {
      throw new Error(`Codex Probe governance violation: ${field}`);
    }
  }
  const configured = agent.allowedTools;
  if (!configured || configured.mode !== 'whitelist' || !Array.isArray(configured.allowed) ||
      configured.allowed.length !== PROBE_TOOLS.length ||
      (configured.exclusions !== undefined && configured.exclusions.length !== 0)) {
    throw new Error('Codex Probe governance requires an exact three-tool whitelist');
  }
  const allowed = new Set(configured.allowed);
  if (allowed.size !== PROBE_TOOLS.length || PROBE_TOOLS.some(tool => !allowed.has(tool)) ||
      configured.allowed.some(tool => typeof tool !== 'string')) {
    throw new Error('Codex Probe governance requires search, extract, and listFiles only');
  }
  if (typeof configured.isEnabled === 'function' &&
      (PROBE_TOOLS.some(tool => configured.isEnabled(tool) !== true) || configured.isEnabled('bash') === true)) {
    throw new Error('Codex Probe tool whitelist is not exact');
  }
  if (!isObject(agent.toolImplementations) || Object.keys(agent.toolImplementations).sort().join(',') !== PROBE_TOOLS.slice().sort().join(',') ||
      PROBE_TOOLS.some(tool => !isObject(agent.toolImplementations[tool]) || typeof agent.toolImplementations[tool].execute !== 'function')) {
    throw new Error('Codex Probe governance requires exact executable Probe tool implementations');
  }
  return { allowed, allowedSet: allowed };
}

function canonicalizePrivateCodexHome(codexHome) {
  if (typeof codexHome !== 'string' || !isAbsolute(codexHome)) {
    throw new Error('codexHome must be an absolute path');
  }
  let canonical;
  let stat;
  try {
    canonical = realpathSync(resolve(codexHome));
    stat = statSync(canonical);
  } catch {
    throw new Error('codexHome must be an existing directory');
  }
  if (!stat.isDirectory() || (stat.mode & 0o077) !== 0) {
    throw new Error('codexHome must be a private directory');
  }
  return canonical;
}

function rejectAmbientCodexConfiguration(cwd) {
  let current = canonicalizeCodexCwd(cwd);
  while (true) {
    const projectConfig = join(current, '.codex', 'config.toml');
    if (existsSync(projectConfig)) throw new Error('Ambient .codex/config.toml is not allowed');
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  if (existsSync('/etc/codex/config.toml') || existsSync('/etc/codex/requirements.toml')) {
    throw new Error('Known system Codex configuration is not allowed');
  }
}

/**
 * Validate without reading the authentication artifact.  The successful
 * capture used one symlink named auth.json and no config file.
 */
export function preflightCodexHome(codexHome, cwd) {
  const canonical = canonicalizePrivateCodexHome(codexHome);
  const entries = readdirSync(canonical, { withFileTypes: true });
  if (entries.length !== 1 || entries[0].name !== 'auth.json') {
    throw new Error('codexHome must be fresh and contain auth.json only');
  }
  const authEntry = entries[0];
  const authPath = join(canonical, 'auth.json');
  if (!authEntry.isSymbolicLink() || !lstatSync(authPath).isSymbolicLink()) {
    throw new Error('codexHome/auth.json must be the captured authentication symlink');
  }
  let target;
  try {
    target = realpathSync(authPath);
    if (!statSync(target).isFile()) throw new Error('not a file');
  } catch {
    throw new Error('codexHome/auth.json symlink target is unavailable');
  }
  // Deliberately use the target only for this safety check.  Never expose it.
  void target;
  rejectAmbientCodexConfiguration(cwd);
  return {
    codexHome: canonical,
    entries: ['auth.json'],
    auth: { present: true, type: 'symlink', targetValidated: true },
    projectConfig: { present: false },
    systemConfig: { configTomlPresent: false, requirementsTomlPresent: false },
    inheritedCredentialEnvironment: false
  };
}

export function buildCodexEnvironment(codexHome, source = process.env) {
  const env = { HOME: codexHome, CODEX_HOME: codexHome };
  for (const key of ALLOWED_ENVIRONMENT) {
    if (typeof source?.[key] === 'string' && source[key].length > 0) env[key] = source[key];
  }
  return env;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function combinePrompts(systemPrompt, customPrompt) {
  if (!systemPrompt && customPrompt) return customPrompt;
  if (systemPrompt && customPrompt) return `${systemPrompt}\n\n## Additional Instructions\n${customPrompt}`;
  return systemPrompt || '';
}

function validatePermissionProfile(profile) {
  if (!exactKeys(profile, ['type', 'file_system', 'network']) || profile.type !== 'managed' || profile.network !== 'restricted') {
    throw new Error('Codex permission_profile is not the captured managed restricted profile');
  }
  if (!exactKeys(profile.file_system, ['type', 'entries']) || profile.file_system.type !== 'restricted' ||
      !Array.isArray(profile.file_system.entries) || profile.file_system.entries.length !== 1) {
    throw new Error('Codex permission_profile.file_system is not the captured root-read profile');
  }
  const entry = profile.file_system.entries[0];
  if (!exactKeys(entry, ['path', 'access']) || entry.access !== 'read' ||
      !exactKeys(entry.path, ['type', 'value']) || entry.path.type !== 'special' ||
      !exactKeys(entry.path.value, ['kind']) || entry.path.value.kind !== 'root') {
    throw new Error('Codex permission profile root-read entry is invalid');
  }
  return cloneJson(profile, 'permission_profile');
}

function validateSessionIdentity(msg, bindings, codexHome) {
  const keys = ['type', 'session_id', 'thread_id', 'model', 'model_provider_id', 'approval_policy',
    'approvals_reviewer', 'permission_profile', 'reasoning_effort', 'rollout_path', 'cwd'];
  if (!isObject(msg) || !Object.keys(msg).every(key => keys.includes(key)) ||
      !requiredKeys(msg, keys) || msg.type !== 'session_configured') {
    throw new Error('Codex session_configured fields do not match the captured shape');
  }
  const identity = {
    type: msg.type,
    session_id: requireString(msg.session_id, 'session_configured.session_id', MAX_ID_LENGTH),
    thread_id: requireString(msg.thread_id, 'session_configured.thread_id', MAX_ID_LENGTH),
    model: requireString(msg.model, 'session_configured.model'),
    model_provider_id: requireString(msg.model_provider_id, 'session_configured.model_provider_id'),
    approval_policy: requireString(msg.approval_policy, 'session_configured.approval_policy'),
    approvals_reviewer: requireString(msg.approvals_reviewer, 'session_configured.approvals_reviewer'),
    permission_profile: validatePermissionProfile(msg.permission_profile),
    reasoning_effort: requireString(msg.reasoning_effort, 'session_configured.reasoning_effort'),
    rollout_path: requireString(msg.rollout_path, 'session_configured.rollout_path', 4096),
    cwd: canonicalizeCodexCwd(msg.cwd)
  };
  if (!isAbsolute(identity.rollout_path) || !isDescendantPath(codexHome, identity.rollout_path) ||
      identity.model !== bindings.model || identity.model_provider_id !== 'openai' ||
      identity.approval_policy !== bindings.approvalPolicy || identity.approvals_reviewer !== 'user' ||
      identity.reasoning_effort !== bindings.thinkingEffort || identity.cwd !== bindings.cwd) {
    throw new Error('Codex session_configured identity does not match requested bindings');
  }
  if (identity.session_id !== identity.thread_id) throw new Error('Codex session/thread identity is not fresh and paired');
  return identity;
}

function validateJsonRpcEventEnvelope(message) {
  if (!exactKeys(message, ['jsonrpc', 'method', 'params']) || message.jsonrpc !== '2.0' || message.method !== 'codex/event') {
    throw new Error('Codex event envelope is not JSON-RPC 2.0 codex/event');
  }
  const params = message.params;
  if (!exactKeys(params, ['_meta', 'id', 'msg']) || !exactKeys(params._meta, ['requestId', 'threadId']) ||
      params._meta.requestId !== 2 || typeof params._meta.threadId !== 'string' || params._meta.threadId.length === 0 ||
      typeof params.id !== 'string') {
    throw new Error('Codex event envelope fields do not match the capture');
  }
  if (!isObject(params.msg) || typeof params.msg.type !== 'string' || params.msg.type.length > MAX_EVENT_TYPE_LENGTH) {
    throw new Error('Codex event message is malformed');
  }
  if (!EVENT_TYPES.has(params.msg.type)) throw new Error(`Codex event type ${params.msg.type} is not observed`);
  return params;
}

function validateInitializeResult(message) {
  if (!exactKeys(message, ['jsonrpc', 'id', 'result']) || message.jsonrpc !== '2.0' || message.id !== 1 ||
      !exactKeys(message.result, ['protocolVersion', 'capabilities', 'serverInfo']) ||
      message.result.protocolVersion !== INITIALIZE_PROTOCOL_VERSION ||
      !exactKeys(message.result.capabilities, ['tools']) || !exactKeys(message.result.capabilities.tools, ['listChanged']) ||
      message.result.capabilities.tools.listChanged !== true ||
      !exactKeys(message.result.serverInfo, ['name', 'title', 'version', 'user_agent']) ||
      message.result.serverInfo.name !== 'codex-mcp-server' || message.result.serverInfo.title !== 'Codex' ||
      message.result.serverInfo.version !== CODEX_PINNED_SERVER_VERSION ||
      typeof message.result.serverInfo.user_agent !== 'string') {
    throw new Error('Codex initialize result shape or version is not the capture');
  }
  return cloneJson(message.result, 'initialize result');
}

function validateUsageObject(value, field) {
  if (!exactKeys(value, ['input_tokens', 'cached_input_tokens', 'output_tokens', 'reasoning_output_tokens', 'total_tokens'])) {
    throw new Error(`Codex ${field} shape is invalid`);
  }
  for (const key of Object.keys(value)) requireNumber(value[key], `${field}.${key}`, { integer: true, min: 0 });
}

function validateTokenCount(msg) {
  if (!exactKeys(msg, ['info', 'rate_limits', 'type']) || msg.type !== 'token_count' ||
      !exactKeys(msg.info, ['total_token_usage', 'last_token_usage', 'model_context_window'])) {
    throw new Error('Codex token_count shape is invalid');
  }
  validateUsageObject(msg.info.total_token_usage, 'token_count.total_token_usage');
  validateUsageObject(msg.info.last_token_usage, 'token_count.last_token_usage');
  requireNumber(msg.info.model_context_window, 'token_count.model_context_window', { integer: true, min: 1 });
  const limits = msg.rate_limits;
  if (!exactKeys(limits, ['limit_id', 'limit_name', 'primary', 'secondary', 'credits', 'individual_limit', 'plan_type', 'rate_limit_reached_type'])) {
    throw new Error('Codex token_count.rate_limits shape is invalid');
  }
  requireString(limits.limit_id, 'token_count.rate_limits.limit_id');
  for (const key of ['limit_name', 'secondary', 'individual_limit', 'rate_limit_reached_type']) {
    if (limits[key] !== null) throw new Error(`Codex token_count.${key} must be null`);
  }
  requireString(limits.plan_type, 'token_count.rate_limits.plan_type');
  if (!exactKeys(limits.primary, ['used_percent', 'window_minutes', 'resets_at'])) throw new Error('Codex primary limits shape is invalid');
  requireNumber(limits.primary.used_percent, 'token_count.used_percent', { min: 0 });
  requireNumber(limits.primary.window_minutes, 'token_count.window_minutes', { integer: true, min: 0 });
  requireNumber(limits.primary.resets_at, 'token_count.resets_at', { integer: true, min: 0 });
  if (!exactKeys(limits.credits, ['has_credits', 'unlimited', 'balance']) ||
      typeof limits.credits.has_credits !== 'boolean' || typeof limits.credits.unlimited !== 'boolean' ||
      typeof limits.credits.balance !== 'string') throw new Error('Codex credits shape is invalid');
}

function validateMcpResult(result) {
  if (!exactKeys(result, ['Ok']) || !exactKeys(result.Ok, ['content']) || !Array.isArray(result.Ok.content)) {
    throw new Error('Codex MCP result must be a terminal Ok content result');
  }
  for (const item of result.Ok.content) {
    if (!exactKeys(item, ['type', 'text']) || item.type !== 'text') throw new Error('Codex MCP result content is invalid');
    requireString(item.text, 'MCP result text');
  }
  return cloneJson(result, 'MCP result');
}

function validateCanonicalMcpResult(result) {
  if (!exactKeys(result, ['content']) || !Array.isArray(result.content) || result.content.length < 1 || result.content.length > 16) {
    throw new Error('Codex canonical MCP result is invalid');
  }
  for (const item of result.content) {
    if (!exactKeys(item, ['type', 'text']) || item.type !== 'text') throw new Error('Codex canonical MCP result content is invalid');
    requireString(item.text, 'canonical MCP result text', MAX_STRING_LENGTH);
  }
  return cloneJson(result, 'canonical MCP result');
}

function validateDuration(duration, field) {
  if (!exactKeys(duration, ['secs', 'nanos']) || !Number.isInteger(duration.secs) || duration.secs < 0 ||
      !Number.isInteger(duration.nanos) || duration.nanos < 0 || duration.nanos > 999999999) {
    throw new Error(`Codex ${field} must be an exact secs/nanos duration`);
  }
  return { secs: duration.secs, nanos: duration.nanos };
}

function digestJson(value, field) {
  const serialized = canonicalJson(value);
  if (typeof serialized !== 'string' || Buffer.byteLength(serialized, 'utf8') > CODEX_MAX_SERIALIZED_BYTES) {
    throw new Error(`Codex ${field} exceeds the serialized-byte bound`);
  }
  return { sha256: createHash('sha256').update(serialized).digest('hex'), bytes: Buffer.byteLength(serialized, 'utf8') };
}

function validateAuditDigest(value, field) {
  if (!exactKeys(value, ['sha256', 'bytes']) || typeof value.sha256 !== 'string' ||
      !/^[a-f0-9]{64}$/.test(value.sha256) || !Number.isSafeInteger(value.bytes) ||
      value.bytes < 0 || value.bytes > CODEX_MAX_SERIALIZED_BYTES) {
    throw new Error(`Codex ${field} digest shape is invalid`);
  }
  return { sha256: value.sha256, bytes: value.bytes };
}

function validateAuditResultDigest(value, field) {
  if (!exactKeys(value, ['sha256', 'bytes', 'status']) ||
      !['ok', 'failed'].includes(value.status)) {
    throw new Error(`Codex ${field} result digest shape is invalid`);
  }
  return { ...validateAuditDigest(value, field), status: value.status };
}

function validateAuditOrdinal(value, field) {
  if (!Number.isSafeInteger(value) || value < 1 || value > CODEX_MAX_EVENT_COUNT) {
    throw new Error(`Codex ${field} ordinal is invalid`);
  }
  return value;
}

function validateDirectAuditMetadata(value, field = 'direct audit metadata') {
  if (!exactKeys(value, [
    'session_id', 'thread_id', 'turn_id', 'sandbox', 'turn_started_at_unix_ms',
    'model', 'reasoning_effort', 'threadId', 'progressToken'
  ]) || !Number.isSafeInteger(value.turn_started_at_unix_ms) || value.turn_started_at_unix_ms <= 0 ||
      !Number.isSafeInteger(value.progressToken) || value.progressToken <= 0) {
    throw new Error(`Codex ${field} shape is invalid`);
  }
  for (const key of ['session_id', 'thread_id', 'turn_id', 'sandbox', 'model', 'reasoning_effort', 'threadId']) {
    requireString(value[key], `${field}.${key}`, MAX_ID_LENGTH);
  }
  return {
    session_id: value.session_id,
    thread_id: value.thread_id,
    turn_id: value.turn_id,
    sandbox: value.sandbox,
    turn_started_at_unix_ms: value.turn_started_at_unix_ms,
    model: value.model,
    reasoning_effort: value.reasoning_effort,
    threadId: value.threadId,
    progressToken: value.progressToken
  };
}

function validateDirectAuditSnapshotShape(snapshot) {
  if (!exactKeys(snapshot, ['starts', 'listCalls', 'toolCalls', 'executionCounts']) ||
      !Array.isArray(snapshot.starts) || !Array.isArray(snapshot.listCalls) || !Array.isArray(snapshot.toolCalls) ||
      !exactKeys(snapshot.executionCounts, PROBE_TOOLS)) {
    throw new Error('Codex governed MCP direct audit snapshot is invalid');
  }
  if (snapshot.starts.length !== 1 || snapshot.listCalls.length !== 1 || snapshot.toolCalls.length > MAX_MCP_CALLS) {
    throw new Error('Codex governed MCP direct audit snapshot record bound is invalid');
  }
  const start = snapshot.starts[0];
  if (!exactKeys(start, ['host', 'port', 'url_path']) || typeof start.host !== 'string' || start.host.length === 0 ||
      start.host.length > MAX_ID_LENGTH || !Number.isSafeInteger(start.port) || start.port < 1 ||
      start.port > 65535 || start.url_path !== '/mcp') {
    throw new Error('Codex direct audit server start record is invalid');
  }
  const list = snapshot.listCalls[0];
  if (!exactKeys(list, ['ordinal', 'tool_names', 'result']) || validateAuditOrdinal(list.ordinal, 'direct audit list') === undefined ||
      !Array.isArray(list.tool_names) || list.tool_names.length !== PROBE_TOOLS.length ||
      !sameJson(list.tool_names, PROBE_TOOLS.map(tool => `${PROBE_TOOL_PREFIX}${tool}`))) {
    throw new Error('Codex direct audit list record is invalid');
  }
  const listDigest = validateAuditDigest(list.result, 'direct audit list result');
  const expectedListDigest = digestJson(EXPECTED_PROBE_TOOL_LIST_RESULT, 'expected Probe tool list result');
  if (!sameJson(listDigest, expectedListDigest)) {
    throw new Error('Codex direct audit list result does not expose the exact Probe tools');
  }
  const ordinals = new Set([list.ordinal]);
  let previousOrdinal = list.ordinal;
  for (const [index, record] of snapshot.toolCalls.entries()) {
    if (!exactKeys(record, ['ordinal', 'name', 'arguments', 'metadata', 'result']) ||
        validateAuditOrdinal(record.ordinal, `direct audit tool[${index}]`) === undefined ||
        ordinals.has(record.ordinal) || record.ordinal <= previousOrdinal ||
        typeof record.name !== 'string' || !PROBE_TOOLS.some(tool => record.name === `${PROBE_TOOL_PREFIX}${tool}`)) {
      throw new Error('Codex direct audit tool record shape or ordinal is invalid');
    }
    validateAuditDigest(record.arguments, `direct audit tool[${index}].arguments`);
    validateDirectAuditMetadata(record.metadata, `direct audit tool[${index}].metadata`);
    validateAuditResultDigest(record.result, `direct audit tool[${index}].result`);
    ordinals.add(record.ordinal);
    previousOrdinal = record.ordinal;
  }
  for (const tool of PROBE_TOOLS) {
    if (!Number.isSafeInteger(snapshot.executionCounts[tool]) || snapshot.executionCounts[tool] < 0 ||
        snapshot.executionCounts[tool] > MAX_MCP_CALLS) {
      throw new Error('Codex direct audit executionCounts value is invalid');
    }
  }
  return snapshot;
}

function directAuditGetter(state) {
  const getter = state.mcpServer?.getGovernedAuditSnapshot || state.mcpServer?.getAuditSnapshot;
  if (typeof getter !== 'function') throw new Error('Codex governed MCP direct audit snapshot is unavailable');
  const snapshot = getter.call(state.mcpServer);
  return validateDirectAuditSnapshotShape(snapshot);
}

function expectedAuditMetadata(state, metadata) {
  const captured = validateDirectAuditMetadata(metadata);
  if (Math.floor(captured.turn_started_at_unix_ms / 1000) !== state.taskStartedAt ||
      (state.turnStartedAtUnixMs !== null && captured.turn_started_at_unix_ms !== state.turnStartedAtUnixMs) ||
      captured.session_id !== state.identity.session_id || captured.thread_id !== state.identity.thread_id ||
      captured.turn_id !== state.turnId || captured.sandbox !== CODEX_EXTENSION_SANDBOX ||
      captured.model !== state.identity.model || captured.reasoning_effort !== state.identity.reasoning_effort ||
      captured.threadId !== state.threadId ||
      (state.lastProgressToken !== 0 && captured.progressToken <= state.lastProgressToken)) {
    throw new Error('Codex direct audit metadata is not bound to this session/thread/turn');
  }
  const expected = {
    session_id: state.identity.session_id,
    thread_id: state.identity.thread_id,
    turn_id: state.turnId,
    sandbox: CODEX_EXTENSION_SANDBOX,
    turn_started_at_unix_ms: captured.turn_started_at_unix_ms,
    model: state.identity.model,
    reasoning_effort: state.identity.reasoning_effort,
    threadId: state.threadId,
    progressToken: captured.progressToken
  };
  state.turnStartedAtUnixMs ??= captured.turn_started_at_unix_ms;
  state.lastProgressToken = captured.progressToken;
  return expected;
}

function crossCheckDirectAudit(state, invocation, result) {
  const snapshot = directAuditGetter(state);
  const argsDigest = digestJson(invocation.arguments, 'MCP arguments');
  const resultDigest = digestJson(result, 'MCP result');
  const index = state.auditConsumedCount;
  if (index >= snapshot.toolCalls.length) throw new Error('Codex canonical MCP call has no unconsumed direct audit record');
  const record = snapshot.toolCalls[index];
  if (!isObject(record) || record.name !== invocation.tool ||
      !isObject(record.arguments) || record.arguments.sha256 !== argsDigest.sha256 || record.arguments.bytes !== argsDigest.bytes ||
      !isObject(record.result) || record.result.sha256 !== resultDigest.sha256 || record.result.bytes !== resultDigest.bytes ||
      record.result.status !== 'ok') {
    throw new Error('Codex direct audit record does not match the canonical MCP call');
  }
  const ordinal = record.ordinal;
  if (!Number.isInteger(ordinal) || ordinal < 1 || state.auditOrdinals.has(ordinal) ||
      (state.lastAuditOrdinal !== 0 && ordinal <= state.lastAuditOrdinal)) {
    throw new Error('Codex direct audit ordinal was reused or out of order');
  }
  const metadata = expectedAuditMetadata(state, record.metadata);
  state.auditOrdinals.add(ordinal);
  state.lastAuditOrdinal = ordinal;
  state.auditConsumedCount++;
  state.directAudit.push({
    ordinal,
    name: invocation.tool,
    arguments: argsDigest,
    result: { ...resultDigest, status: 'ok' },
    metadata
  });
}

function validateTerminalDirectAudit(state) {
  const snapshot = directAuditGetter(state);
  if (snapshot.toolCalls.length !== state.auditConsumedCount) {
    throw new Error('Codex direct audit contains extra or unconsumed tool records');
  }
  const expectedCounts = { search: 0, extract: 0, listFiles: 0 };
  for (const record of state.directAudit) {
    const bare = record.name.slice(PROBE_TOOL_PREFIX.length);
    if (!Object.prototype.hasOwnProperty.call(expectedCounts, bare)) throw new Error('Codex direct audit tool is not allowlisted');
    expectedCounts[bare]++;
  }
  if (!sameJson(snapshot.executionCounts, expectedCounts)) {
    throw new Error('Codex direct audit executionCounts do not match consumed records');
  }
  if (state.directAudit.length > 0 && snapshot.listCalls[0].ordinal >= state.directAudit[0].ordinal) {
    throw new Error('Codex direct audit list ordinal is not ordered before tool calls');
  }
}

function validateMcpInvocation(invocation, serverName, allowedSet) {
  if (!exactKeys(invocation, ['server', 'tool', 'arguments']) || invocation.server !== serverName ||
      typeof invocation.tool !== 'string' || !invocation.tool.startsWith(PROBE_TOOL_PREFIX)) {
    throw new Error('Codex MCP invocation identity is not exact');
  }
  const bare = invocation.tool.slice(PROBE_TOOL_PREFIX.length);
  if (!allowedSet.has(bare) || invocation.tool !== `${PROBE_TOOL_PREFIX}${bare}` || !isObject(invocation.arguments)) {
    throw new Error('Codex MCP invocation is not an exact Probe call');
  }
  cloneJson(invocation.arguments, 'MCP arguments');
  return { server: invocation.server, tool: invocation.tool, arguments: cloneJson(invocation.arguments, 'MCP arguments') };
}

function validateRawMessageItem(state, item) {
  if (!isObject(item) || item.type !== 'message' || !Array.isArray(item.content) || item.content.length !== 1 ||
      !exactKeys(item.content[0], ['type', 'text'])) throw new Error('Codex raw_response_item message shape is invalid');
  const part = item.content[0];
  requireString(part.text, 'raw_response_item.text');
  if (item.role === 'developer' || item.role === 'user') {
    if (!exactKeys(item, ['content', 'internal_chat_message_metadata_passthrough', 'role', 'type']) || part.type !== 'input_text' ||
        !exactKeys(item.internal_chat_message_metadata_passthrough, ['turn_id']) ||
        item.internal_chat_message_metadata_passthrough.turn_id !== state.turnId) {
      throw new Error('Codex raw input message shape or turn_id is invalid');
    }
    if (state.rawMessageRoles.length >= 16 || (item.role === 'developer' && state.rawMessageRoles.includes('developer')) ||
        (item.role === 'developer' && state.rawMessageRoles.length !== 0) ||
        (item.role === 'user' && state.rawMessageRoles.length > 0 && state.rawMessageRoles[state.rawMessageRoles.length - 1] === 'assistant')) {
      throw new Error('Codex raw input message bound or order is invalid');
    }
    state.rawMessageRoles.push(item.role);
    return { kind: item.role, text: part.text };
  }
  if (item.role === 'assistant') {
    if (!exactKeys(item, ['content', 'id', 'internal_chat_message_metadata_passthrough', 'phase', 'role', 'type']) ||
        part.type !== 'output_text' || item.phase !== 'final_answer' ||
        !exactKeys(item.internal_chat_message_metadata_passthrough, ['turn_id']) ||
        item.internal_chat_message_metadata_passthrough.turn_id !== state.turnId) {
      throw new Error('Codex raw assistant message shape or turn_id is invalid');
    }
    requireString(item.id, 'raw assistant id', MAX_ID_LENGTH);
    if (item.id !== state.agentItemId || item.content[0].text !== state.agentMessage) {
      throw new Error('Codex raw assistant message is not bound to the final AgentMessage');
    }
    return { kind: 'assistant', id: item.id, text: part.text };
  }
  throw new Error('Codex raw_response_item role is not observed');
}

function addActivity(state, activity) {
  if (state.activity.length >= MAX_ACTIVITY) throw new Error('Codex activity bound exceeded');
  state.activity.push(cloneJson(activity, 'activity record', CODEX_MAX_SERIALIZED_BYTES));
}

function validateRawReasoningItem(state, item) {
  if (!exactKeys(item, ['encrypted_content', 'id', 'internal_chat_message_metadata_passthrough', 'summary', 'type']) ||
      item.type !== 'reasoning' || !exactKeys(item.internal_chat_message_metadata_passthrough, ['turn_id']) ||
      item.internal_chat_message_metadata_passthrough.turn_id !== state.turnId || !Array.isArray(item.summary)) {
    throw new Error('Codex raw reasoning item shape or turn_id is invalid');
  }
  requireString(item.id, 'raw reasoning id', MAX_ID_LENGTH);
  requireString(item.encrypted_content, 'raw reasoning content', MAX_STRING_LENGTH);
  cloneJson(item.summary, 'raw reasoning summary');
  return item.id;
}

function validateBridgeCallItem(state, item) {
  if (!exactKeys(item, ['call_id', 'id', 'input', 'internal_chat_message_metadata_passthrough', 'name', 'status', 'type']) ||
      item.type !== 'custom_tool_call' || item.name !== 'exec' || item.status !== 'completed' ||
      !exactKeys(item.internal_chat_message_metadata_passthrough, ['turn_id']) ||
      item.internal_chat_message_metadata_passthrough.turn_id !== state.turnId) {
    throw new Error('Codex raw exec bridge call shape is invalid');
  }
  const id = requireString(item.id, 'raw exec bridge item id', MAX_ID_LENGTH);
  const callId = requireString(item.call_id, 'raw exec bridge call_id', MAX_ID_LENGTH);
  if (state.activeBridge || state.seenItemIds.has(id) || state.seenItemIds.has(callId) ||
      state.seenBridgeCallIds.has(callId) || state.seenBridgeItemIds.has(id) ||
      state.seenBridgeCallIds.has(id) || state.seenBridgeItemIds.has(callId) ||
      id === callId || state.seenNestedCallIds.has(id) || state.seenNestedCallIds.has(callId)) {
    throw new Error('Codex raw exec bridge call or item identity was reused');
  }
  if (state.seenBridgeCallIds.size >= MAX_BRIDGE_CALLS) throw new Error('Codex bridge call bound exceeded');
  if (typeof item.input !== 'string' || item.input.length === 0 || Buffer.byteLength(item.input, 'utf8') > MAX_STRING_LENGTH) {
    throw new Error('Codex raw exec bridge input is invalid');
  }
  const bytes = Buffer.byteLength(item.input, 'utf8');
  const inputHash = createHash('sha256').update(item.input).digest('hex');
  state.seenBridgeCallIds.add(callId);
  state.seenBridgeItemIds.add(id);
  const bridge = {
    outerCallId: callId,
    outerItemId: id,
    inputHash,
    inputBytes: bytes,
    status: 'open',
    nestedCallId: null,
    nestedItemId: null
  };
  state.bridge.set(callId, bridge);
  state.activeBridge = bridge;
  addActivity(state, { kind: 'exec_bridge', outerCallId: callId, outerItemId: id, inputHash, inputBytes: bytes, status: 'open' });
}

function validateBridgeOutputItem(state, item) {
  if (!exactKeys(item, ['call_id', 'internal_chat_message_metadata_passthrough', 'output', 'type']) ||
      item.type !== 'custom_tool_call_output' || !exactKeys(item.internal_chat_message_metadata_passthrough, ['turn_id']) ||
      item.internal_chat_message_metadata_passthrough.turn_id !== state.turnId || !Array.isArray(item.output) ||
      item.output.length < 1 || item.output.length > 8) {
    throw new Error('Codex raw exec bridge output shape is invalid');
  }
  const callId = requireString(item.call_id, 'raw exec bridge output call_id', MAX_ID_LENGTH);
  const bridge = state.bridge.get(callId);
  if (!bridge || state.activeBridge !== bridge || bridge.outputHash) throw new Error('Codex raw exec bridge output has no matching call');
  if (bridge.nestedCallId !== null && (!bridge.nestedCompleted || !bridge.legacyEnded)) {
    throw new Error('Codex raw exec bridge output arrived before nested MCP completion');
  }
  for (const part of item.output) {
    if (!exactKeys(part, ['text', 'type']) || part.type !== 'input_text') throw new Error('Codex raw exec bridge output part is invalid');
    requireString(part.text, 'raw exec bridge output text', MAX_STRING_LENGTH);
  }
  const serialized = JSON.stringify(item.output);
  const bytes = Buffer.byteLength(serialized, 'utf8');
  if (bytes > CODEX_MAX_SERIALIZED_BYTES) throw new Error('Codex raw exec bridge output exceeds the bound');
  bridge.outputHash = createHash('sha256').update(serialized).digest('hex');
  bridge.outputBytes = bytes;
  bridge.outputParts = item.output.length;
  bridge.status = 'completed';
  state.bridgeReceipt.push({
    outerCallId: bridge.outerCallId,
    outerItemId: bridge.outerItemId,
    inputHash: bridge.inputHash,
    inputBytes: bridge.inputBytes,
    outputHash: bridge.outputHash,
    outputBytes: bridge.outputBytes,
    nestedCallId: bridge.nestedCallId,
    nestedItemId: bridge.nestedItemId,
    status: 'completed'
  });
  addActivity(state, { kind: 'exec_bridge_output', outerCallId: bridge.outerCallId, outerItemId: bridge.outerItemId,
    outputHash: bridge.outputHash, outputBytes: bytes, nestedCallId: bridge.nestedCallId, status: 'completed' });
  state.bridge.delete(callId);
  state.activeBridge = null;
}

function validateLifecycleItem(state, item, kind) {
  if (kind === 'UserMessage') {
    if (!exactKeys(item, ['content', 'id', 'type']) || item.type !== kind || !Array.isArray(item.content) || item.content.length !== 1 ||
        !exactKeys(item.content[0], ['text', 'text_elements', 'type']) || item.content[0].type !== 'text' ||
        !Array.isArray(item.content[0].text_elements) || item.content[0].text !== state.prompt) {
      throw new Error('Codex UserMessage lifecycle shape is invalid');
    }
    requireString(item.id, 'UserMessage id', MAX_ID_LENGTH);
    return cloneJson(item, 'UserMessage lifecycle item');
  }
  if (kind === 'Reasoning') {
    if (!exactKeys(item, ['id', 'raw_content', 'summary_text', 'type']) || item.type !== kind ||
        !Array.isArray(item.summary_text) || !Array.isArray(item.raw_content)) {
      throw new Error('Codex Reasoning lifecycle item shape is invalid');
    }
    requireString(item.id, 'Reasoning id', MAX_ID_LENGTH);
    return cloneJson(item, 'Reasoning lifecycle item');
  }
  if (kind === 'McpToolCall') {
    if (!exactKeys(item, ['arguments', 'id', 'server', 'status', 'tool', 'type']) || item.type !== kind ||
        item.status !== 'inProgress' || item.server !== state.serverName || typeof item.tool !== 'string' ||
        !isObject(item.arguments)) throw new Error('Codex McpToolCall start shape is invalid');
    requireString(item.id, 'McpToolCall id', MAX_ID_LENGTH);
    cloneJson(item.arguments, 'McpToolCall arguments');
    return cloneJson(item, 'McpToolCall lifecycle item');
  }
  if (!exactKeys(item, ['content', 'id', 'phase', 'type']) || item.type !== 'AgentMessage' || item.phase !== 'final_answer' ||
      !Array.isArray(item.content) || item.content.length !== 1 || !exactKeys(item.content[0], ['text', 'type']) ||
      item.content[0].type !== 'Text' || item.content[0].text !== '') {
    throw new Error('Codex AgentMessage lifecycle start shape is invalid');
  }
  requireString(item.id, 'AgentMessage id', MAX_ID_LENGTH);
  return cloneJson(item, 'AgentMessage lifecycle item');
}

function validateTaskStarted(msg) {
  if (!exactKeys(msg, ['collaboration_mode_kind', 'model_context_window', 'started_at', 'turn_id', 'type']) ||
      msg.type !== 'task_started' || msg.collaboration_mode_kind !== 'default') throw new Error('Codex task_started shape is invalid');
  requireNumber(msg.started_at, 'task_started.started_at', { integer: true, min: 0 });
  requireNumber(msg.model_context_window, 'task_started.model_context_window', { integer: true, min: 1 });
  return {
    turnId: requireString(msg.turn_id, 'task_started.turn_id', MAX_ID_LENGTH),
    startedAt: msg.started_at
  };
}

function validateTaskComplete(msg, state) {
  if (!exactKeys(msg, ['completed_at', 'duration_ms', 'last_agent_message', 'time_to_first_token_ms', 'turn_id', 'type']) ||
      msg.type !== 'task_complete' || msg.turn_id !== state.turnId || msg.last_agent_message !== state.agentMessage) {
    throw new Error('Codex task_complete shape or message binding is invalid');
  }
  requireNumber(msg.completed_at, 'task_complete.completed_at', { integer: true, min: 0 });
  requireNumber(msg.duration_ms, 'task_complete.duration_ms', { integer: true, min: 0 });
  requireNumber(msg.time_to_first_token_ms, 'task_complete.time_to_first_token_ms', { integer: true, min: 0 });
}

function validateOuterResult(result, state) {
  if (!state.taskCompleteSeen || state.openItems.size !== 0 || state.mcp.size !== 0 || state.bridge.size !== 0 || state.toolSuccessCount < 1 ||
      !exactKeys(result, ['content', 'structuredContent']) || !Array.isArray(result.content) || result.content.length !== 1 ||
      !exactKeys(result.content[0], ['text', 'type']) || result.content[0].type !== 'text' ||
      !exactKeys(result.structuredContent, ['content', 'threadId']) || result.structuredContent.threadId !== state.threadId ||
      result.structuredContent.content !== result.content[0].text || result.content[0].text !== state.agentMessage ||
      result.content[0].text !== state.taskCompleteMessage) {
    throw new Error('Codex result content/thread binding is invalid');
  }
  requireString(result.content[0].text, 'result content');
  return cloneJson(result, 'result');
}

function validateAgentMessage(msg, state) {
  if (!exactKeys(msg, ['memory_citation', 'message', 'phase', 'type']) || msg.type !== 'agent_message' ||
      msg.phase !== 'final_answer' || msg.memory_citation !== null || msg.message !== state.deltaText) {
    throw new Error('Codex agent_message shape or delta binding is invalid');
  }
  state.agentMessage = requireString(msg.message, 'agent_message.message');
}

function validateUserMessage(msg, state) {
  if (!exactKeys(msg, ['images', 'local_images', 'message', 'text_elements', 'type']) || msg.type !== 'user_message' ||
      msg.message !== state.prompt || !Array.isArray(msg.images) || !Array.isArray(msg.local_images) || !Array.isArray(msg.text_elements) ||
      msg.images.length !== 0 || msg.local_images.length !== 0 || msg.text_elements.length !== 0) {
    throw new Error('Codex user_message shape or prompt binding is invalid');
  }
}

function validateEventState(state, params, serverName, allowedSet) {
  const type = params.msg.type;
  if (!EVENT_TYPES.has(type)) throw new Error(`Codex event type ${type} is denied`);
  if (state.resultSeen || state.taskCompleteSeen) throw new Error('Codex traffic arrived after terminal state');
  if (state.activeBridge) {
    const nestedCall = state.activeBridge.nestedCallId === null
      ? null
      : state.mcp.get(state.activeBridge.nestedCallId);
    const isOutput = type === 'raw_response_item' && params.msg.item?.type === 'custom_tool_call_output';
    const isNestedStart = type === 'item_started' && params.msg.item?.type === 'McpToolCall' &&
      state.activeBridge.nestedCallId === null;
    const isNestedBegin = type === 'mcp_tool_call_begin' && nestedCall && !nestedCall.legacyBegun;
    const isNestedCompletion = type === 'item_completed' && params.msg.item?.type === 'McpToolCall' &&
      nestedCall && params.msg.item.id === nestedCall.callId && nestedCall.legacyBegun && !nestedCall.canonicalCompleted;
    const isNestedEnd = type === 'mcp_tool_call_end' && nestedCall && nestedCall.canonicalCompleted && !nestedCall.legacyEnded;
    if (!isOutput && !isNestedStart && !isNestedBegin && !isNestedCompletion && !isNestedEnd) {
      throw new Error('Codex exec bridge has unrelated interleaved traffic');
    }
  } else if (type === 'mcp_tool_call_begin' || type === 'mcp_tool_call_end' ||
      (type === 'item_started' && params.msg.item?.type === 'McpToolCall') ||
      (type === 'item_completed' && params.msg.item?.type === 'McpToolCall') ||
      (type === 'raw_response_item' && params.msg.item?.type === 'custom_tool_call_output')) {
    throw new Error('Codex nested MCP traffic has no open exec bridge');
  }
  if (type === 'session_configured' || type === 'mcp_startup_update' || type === 'mcp_startup_complete') {
    if (params.id !== '') throw new Error('Codex pre-turn event id must be empty');
  } else if (type === 'task_started') {
    if (params.id !== params.msg.turn_id) throw new Error('Codex task_started id does not equal its turn_id');
  } else if (state.turnId === null || params.id !== state.turnId) {
    throw new Error('Codex event id does not equal the real turn_id');
  }
  if (type !== 'session_configured' && params._meta.threadId !== state.threadId) throw new Error('Codex event thread_id mismatch');
  state.eventCounts[type] = (state.eventCounts[type] || 0) + 1;
  if (Object.values(state.eventCounts).reduce((sum, value) => sum + value, 0) > CODEX_MAX_EVENT_COUNT) {
    throw new Error('Codex event count exceeds the bound');
  }

  if (type === 'session_configured') {
    if (state.identity || Object.keys(state.eventCounts).some(key => key !== type)) throw new Error('Codex session_configured is duplicated or out of order');
    state.identity = validateSessionIdentity(params.msg, state.bindings, state.codexHome);
    state.threadId = identityThread(state.identity, params._meta.threadId);
    return;
  }
  if (!state.identity) throw new Error('Codex event preceded session_configured');

  if (type === 'mcp_startup_update') {
    if (!exactKeys(params.msg, ['server', 'status', 'type']) || params.msg.type !== type || params.msg.server !== serverName ||
        !exactKeys(params.msg.status, ['state']) || !['starting', 'ready'].includes(params.msg.status.state) || state.startupComplete) {
      throw new Error('Codex MCP startup update is invalid');
    }
    if (params.msg.status.state === 'starting') {
      if (state.startingSeen) throw new Error('Codex MCP startup starting update was duplicated');
      state.startingSeen = true;
    } else {
      if (state.readySeen || (state.startingSeen === false && state.startupUpdateCount > 0)) throw new Error('Codex MCP startup ready update is invalid');
      state.readySeen = true;
      state.readyDynamic = true;
    }
    state.startupUpdateCount++;
    if (state.startupUpdateCount > 2) throw new Error('Codex MCP startup update bound exceeded');
    return;
  }
  if (type === 'mcp_startup_complete') {
    if (state.startupComplete || !exactKeys(params.msg, ['cancelled', 'failed', 'ready', 'type']) || params.msg.type !== type ||
        !Array.isArray(params.msg.ready) || !Array.isArray(params.msg.failed) || !Array.isArray(params.msg.cancelled) ||
        params.msg.failed.length !== 0 || params.msg.cancelled.length !== 0 ||
        params.msg.ready.some(name => typeof name !== 'string') || params.msg.ready.length > 1 ||
        (params.msg.ready.length === 1 && params.msg.ready[0] !== serverName)) {
      throw new Error('Codex mcp_startup_complete arrays are not exact');
    }
    state.startupComplete = true;
    state.readyDynamic = params.msg.ready.length === 1 && params.msg.ready[0] === serverName;
    if (state.readyDynamic && !state.readySeen && state.startupUpdateCount > 0) throw new Error('Codex MCP startup complete skipped ready update');
    return;
  }
  if (type === 'task_started') {
    if (state.taskStarted || state.turnId !== null) throw new Error('Codex task_started is duplicated or out of order');
    const task = validateTaskStarted(params.msg);
    state.turnId = task.turnId;
    state.taskStartedAt = task.startedAt;
    if (params._meta.threadId !== state.threadId) throw new Error('Codex task_started thread mismatch');
    state.taskStarted = true;
    return;
  }
  if (!state.taskStarted || state.startupComplete === false) throw new Error('Codex event preceded task/startup completion');

  if (type === 'raw_response_item') {
    if (!exactKeys(params.msg, ['item', 'type']) || params.msg.type !== type || !isObject(params.msg.item)) throw new Error('Codex raw_response_item wrapper is invalid');
    const itemType = params.msg.item.type;
    if (itemType === 'message') {
      const parsed = validateRawMessageItem(state, params.msg.item);
      if (parsed.kind === 'developer' || parsed.kind === 'user') {
        if (state.userItemStarted) throw new Error('Codex raw input message arrived after UserMessage lifecycle');
      } else {
        if (!state.agentCompleted || state.rawAssistantSeen) throw new Error('Codex raw assistant message is out of order');
        state.rawAssistantSeen = true;
      }
      return;
    }
    if (itemType === 'reasoning') {
      const reasoningId = validateRawReasoningItem(state, params.msg.item);
      if (!state.completedReasoningIds.has(reasoningId) || state.rawReasoningIds.has(reasoningId) ||
          !state.reasoningItems.has(reasoningId)) throw new Error('Codex raw reasoning item is not paired');
      state.rawReasoningIds.add(reasoningId);
      return;
    }
    if (itemType === 'custom_tool_call') {
      validateBridgeCallItem(state, params.msg.item);
      return;
    }
    if (itemType === 'custom_tool_call_output') {
      validateBridgeOutputItem(state, params.msg.item);
      return;
    }
    throw new Error(`Codex raw response item type ${String(itemType)} is denied`);
  }

  if (type === 'item_started') {
    if (!exactKeys(params.msg, ['item', 'started_at_ms', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId) throw new Error('Codex item start wrapper is invalid');
    requireNumber(params.msg.started_at_ms, 'item.started_at_ms', { integer: true, min: 0 });
    if (!isObject(params.msg.item) || !['UserMessage', 'Reasoning', 'McpToolCall', 'AgentMessage'].includes(params.msg.item.type)) {
      throw new Error('Codex native item type is denied');
    }
    if (state.seenItemIds.has(params.msg.item.id) || state.seenBridgeCallIds.has(params.msg.item.id) ||
        state.seenBridgeItemIds.has(params.msg.item.id) || state.seenNestedCallIds.has(params.msg.item.id) ||
        state.seenItemIds.size >= MAX_ITEMS) throw new Error('Codex item id was reused or bound exceeded');
    const itemType = params.msg.item.type;
    if (itemType === 'UserMessage' && state.userItemStarted) throw new Error('Codex UserMessage lifecycle was duplicated');
    if (itemType === 'AgentMessage' && (state.agentItemStarted || !state.userMessageSeen)) throw new Error('Codex AgentMessage lifecycle is out of order');
    if (itemType === 'McpToolCall' && (!state.startupComplete || !state.readyDynamic || state.mcp.size >= MAX_MCP_CALLS ||
        state.seenNestedCallIds.size >= MAX_MCP_CALLS ||
        !state.activeBridge || state.activeBridge.nestedCallId !== null)) {
      throw new Error('Codex MCP tool call arrived before the dynamic server was ready');
    }
    const item = validateLifecycleItem(state, params.msg.item, itemType);
    state.seenItemIds.add(item.id);
    state.openItems.set(item.id, { type: itemType, item });
    if (itemType === 'UserMessage') { state.userItemStarted = true; state.userItem = item; }
    if (itemType === 'Reasoning') { state.reasoningItems.set(item.id, item); }
    if (itemType === 'AgentMessage') { state.agentItemStarted = true; state.agentItemId = item.id; state.agentItem = item; }
    if (itemType === 'McpToolCall') {
      const invocation = validateMcpInvocation({ server: item.server, tool: item.tool, arguments: item.arguments }, serverName, allowedSet);
      if (item.id === state.activeBridge.outerCallId || item.id === state.activeBridge.outerItemId ||
          state.seenBridgeCallIds.has(item.id) || state.seenBridgeItemIds.has(item.id)) {
        throw new Error('Codex nested MCP call identity equals an outer bridge identity');
      }
      state.seenNestedCallIds.add(item.id);
      state.activeBridge.nestedCallId = item.id;
      state.activeBridge.nestedItemId = item.id;
      state.mcp.set(item.id, { callId: item.id, itemId: item.id, outerCallId: state.activeBridge.outerCallId,
        invocation, canonicalStarted: true, legacyBegun: false, canonicalCompleted: false, legacyEnded: false });
    }
    return;
  }

  if (type === 'item_completed') {
    if (!exactKeys(params.msg, ['completed_at_ms', 'item', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId || !isObject(params.msg.item)) throw new Error('Codex item completion wrapper is invalid');
    requireNumber(params.msg.completed_at_ms, 'item.completed_at_ms', { integer: true, min: 0 });
    const item = params.msg.item;
    const open = state.openItems.get(item.id);
    if (!open || open.type !== item.type) throw new Error('Codex item completion has no matching start');
    if (item.type === 'UserMessage' || item.type === 'Reasoning') {
      if (!sameJson(item, open.item)) throw new Error(`Codex ${item.type} completion is not paired`);
      state.openItems.delete(item.id);
      if (item.type === 'UserMessage') state.userItemCompleted = true;
      else state.completedReasoningIds.add(item.id);
      return;
    }
    if (item.type === 'AgentMessage') {
      if (!exactKeys(item, ['content', 'id', 'phase', 'type']) || item.phase !== 'final_answer' || !Array.isArray(item.content) ||
          item.content.length !== 1 || !exactKeys(item.content[0], ['text', 'type']) || item.content[0].type !== 'Text' ||
          item.content[0].text !== state.deltaText || !sameJson(item, { ...open.item, content: item.content })) throw new Error('Codex AgentMessage completion is invalid');
      state.agentCompleted = true;
      state.agentMessage = requireString(item.content[0].text, 'AgentMessage text');
      state.openItems.delete(item.id);
      return;
    }
    if (item.type === 'McpToolCall') {
      const call = state.mcp.get(item.id);
      if (!call || call.canonicalCompleted || !exactKeys(item, ['arguments', 'duration', 'id', 'result', 'server', 'status', 'tool', 'type']) ||
          item.status !== 'completed' || item.server !== call.invocation.server || item.tool !== call.invocation.tool || !sameJson(item.arguments, call.invocation.arguments)) {
        throw new Error('Codex canonical MCP completion is invalid');
      }
      const duration = validateDuration(item.duration, 'canonical MCP duration');
      const result = validateCanonicalMcpResult(item.result);
      crossCheckDirectAudit(state, call.invocation, result);
      call.canonicalCompleted = true;
      call.duration = duration;
      call.result = result;
      const bridge = state.activeBridge;
      if (!bridge || bridge.nestedCallId !== call.callId || bridge.outerCallId === call.callId) {
        throw new Error('Codex canonical MCP call is not associated with its outer bridge');
      }
      bridge.nestedCompleted = true;
      state.openItems.delete(item.id);
      addActivity(state, { kind: 'canonical_mcp', callId: item.id, outerCallId: call.outerCallId, server: call.invocation.server, tool: call.invocation.tool,
        arguments: digestJson(call.invocation.arguments, 'MCP arguments'), result: digestJson(result, 'MCP result'), duration });
      return;
    }
  }

  if (type === 'user_message') {
    if (state.userMessageSeen || !state.userItemCompleted || state.rawMessageRoles.filter(role => role === 'user').length < 1) throw new Error('Codex user_message is out of order');
    validateUserMessage(params.msg, state);
    state.userMessageSeen = true;
    return;
  }
  if (type === 'mcp_tool_call_begin') {
    if (!state.readyDynamic || !exactKeys(params.msg, ['call_id', 'invocation', 'type'])) throw new Error('Codex MCP begin shape is invalid');
    const callId = requireString(params.msg.call_id, 'MCP begin call_id', MAX_ID_LENGTH);
    const call = state.mcp.get(callId);
    if (!call || call.legacyBegun || !sameJson(call.invocation, validateMcpInvocation(params.msg.invocation, serverName, allowedSet))) throw new Error('Codex MCP begin is not paired with canonical start');
    call.legacyBegun = true;
    return;
  }
  if (type === 'mcp_tool_call_end') {
    if (!exactKeys(params.msg, ['call_id', 'duration', 'invocation', 'result', 'type'])) throw new Error('Codex MCP end shape is invalid');
    const callId = requireString(params.msg.call_id, 'MCP end call_id', MAX_ID_LENGTH);
    const call = state.mcp.get(callId);
    const invocation = validateMcpInvocation(params.msg.invocation, serverName, allowedSet);
    if (!call || !call.legacyBegun || call.legacyEnded || !call.canonicalCompleted || !sameJson(call.invocation, invocation)) throw new Error('Codex MCP end is not paired with canonical lifecycle');
    const duration = validateDuration(params.msg.duration, 'legacy MCP duration');
    const result = validateMcpResult(params.msg.result);
    if (!sameJson(duration, call.duration) || !sameJson(result.Ok, call.result) || !sameJson(result.Ok.content, call.result.content)) {
      throw new Error('Codex canonical and legacy MCP results do not match');
    }
    call.legacyEnded = true;
    if (!state.activeBridge || state.activeBridge.nestedCallId !== callId || !call.canonicalCompleted) {
      throw new Error('Codex legacy MCP call is not associated with its outer bridge');
    }
    state.activeBridge.legacyEnded = true;
    state.mcp.delete(callId);
    state.toolSuccessCount++;
    const directAudit = state.directAudit[state.directAudit.length - 1];
    if (!directAudit || directAudit.name !== invocation.tool) throw new Error('Codex direct audit pairing is incomplete');
    state.mcpReceipt.push({
      callId,
      nestedCallId: callId,
      nestedItemId: call.itemId,
      outerCallId: call.outerCallId,
      outerItemId: state.activeBridge?.outerItemId || null,
      server: invocation.server,
      tool: invocation.tool,
      arguments: digestJson(invocation.arguments, 'MCP arguments'),
      result: { ...digestJson(call.result, 'MCP result'), status: 'ok' },
      duration,
      outcome: 'ok',
      directAudit: { ordinal: directAudit.ordinal, metadata: directAudit.metadata }
    });
    addActivity(state, { kind: 'legacy_mcp', callId, outerCallId: call.outerCallId, server: invocation.server, tool: invocation.tool, duration,
      arguments: digestJson(invocation.arguments, 'MCP arguments'), result: digestJson(call.result, 'MCP result'), outcome: 'ok' });
    return;
  }
  if (type === 'agent_message_content_delta') {
    if (!state.agentItemStarted || state.agentCompleted || !exactKeys(params.msg, ['delta', 'item_id', 'thread_id', 'turn_id', 'type']) ||
        params.msg.item_id !== state.agentItemId || params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId) {
      throw new Error('Codex agent_message_content_delta binding is invalid');
    }
    state.deltaText += requireString(params.msg.delta, 'agent_message_content_delta.delta');
    if (state.deltaText.length > MAX_STRING_LENGTH) throw new Error('Codex response exceeds the bound');
    return;
  }
  if (type === 'agent_message') {
    if (!state.agentCompleted || state.agentMessageSeen) throw new Error('Codex agent_message is out of order');
    validateAgentMessage(params.msg, state);
    state.agentMessageSeen = true;
    return;
  }
  if (type === 'token_count') {
    if (++state.tokenCount > MAX_TOKEN_COUNTS) throw new Error('Codex token_count bound exceeded');
    validateTokenCount(params.msg);
    return;
  }
  if (type === 'task_complete') {
    if (state.taskCompleteSeen || state.openItems.size !== 0 || state.mcp.size !== 0 || state.bridge.size !== 0 ||
        !state.userMessageSeen || !state.agentMessageSeen || !state.rawAssistantSeen || state.toolSuccessCount < 1 ||
        !state.startupComplete || !state.readyDynamic || state.reasoningItems.size === 0 ||
        state.completedReasoningIds.size !== state.reasoningItems.size ||
        state.rawReasoningIds.size !== state.reasoningItems.size || state.activeBridge) {
      throw new Error('Codex task_complete lifecycle admission is incomplete');
    }
    validateTaskComplete(params.msg, state);
    state.taskCompleteMessage = params.msg.last_agent_message;
    state.taskCompleteSeen = true;
    validateTerminalDirectAudit(state);
    return;
  }
  throw new Error(`Codex ${type} event is out of the observed bounded lifecycle`);
}

function identityThread(identity, metadataThread) {
  if (identity.thread_id !== metadataThread) throw new Error('Codex session_configured metadata thread mismatch');
  return identity.thread_id;
}

export async function createCodexEngine(options = {}) {
  const bindings = validateCodexBindings(options);
  const governance = validateGovernedAgent(options.agent);
  const executable = verifyExecutable({
    executablePath: options.codexExecutablePath,
    expectedExecutablePath: options.codexExpectedExecutablePath,
    expectedExecutableSha256: options.codexExpectedExecutableSha256
  });
  const expectedServerVersion = options.codexExpectedServerVersion ?? CODEX_PINNED_SERVER_VERSION;
  if (expectedServerVersion !== CODEX_PINNED_SERVER_VERSION) throw new Error('Codex server version pin is invalid');
  const isolation = preflightCodexHome(options.codexHome, bindings.cwd);

  const session = new Session(options.sessionId || randomBytes(8).toString('hex'), options.debug);
  const serverName = `probe_${session.id}`;
  let mcpServer = null;
  let codexProcess = null;
  let reader = null;
  let initializePending = null;
  let queryPending = null;
  let state = null;
  let initializeVersion = null;
  let requestSent = false;
  let queryReserved = false;
  let initializeTimer = null;
  let turnTimer = null;
  let poisoned = false;
  let poisonError = null;
  let closed = false;
  let cleanupPromise = null;
  let cleanupOutcome = { status: 'not_started' };
  let mcpStartPromise = null;
  let mcpStartTimedOut = false;
  let mcpStartResolved = false;
  let mcpLateStopPromise = null;
  let childExited = false;
  let childExitCode = null;
  let childExitSignal = null;
  let stderr = Buffer.alloc(0);
  let incomingBytes = 0;

  const requested = buildCodexRequestedMetadata(bindings);

  function safeReceipt(currentState = state) {
    const outerDigest = currentState?.result?.content?.[0]?.text
      ? digestJson(currentState.result.content[0].text, 'outer content')
      : null;
    const activityDigest = currentState ? digestJson(currentState.activity, 'activity') : null;
    const receipt = {
      requested,
      effective: currentState?.identity ? {
        session_id: currentState.identity.session_id,
        thread_id: currentState.identity.thread_id,
        model: currentState.identity.model,
        reasoning_effort: currentState.identity.reasoning_effort,
        sandbox: currentState.bindings.sandbox,
        approval_policy: currentState.identity.approval_policy,
        approvals_reviewer: currentState.identity.approvals_reviewer,
        cwd: currentState.identity.cwd,
        rollout_path: currentState.identity.rollout_path
      } : null,
      ids: currentState ? { session_id: currentState.identity?.session_id || null, thread_id: currentState.threadId, turn_id: currentState.turnId } : {},
      executable: { path: executable.path, sha256: executable.sha256 },
      initialize: { serverInfoVersion: initializeVersion },
      bounds: currentState ? {
        incomingBytes: currentState.incomingBytes,
        maxIncomingBytes: CODEX_MAX_INCOMING_BYTES,
        eventCount: Object.values(currentState.eventCounts).reduce((sum, value) => sum + value, 0),
        maxEventCount: CODEX_MAX_EVENT_COUNT,
        eventBytes: currentState.incomingBytes,
        maxEventBytes: CODEX_MAX_INCOMING_BYTES,
        openItems: currentState.openItems.size,
        nestedCalls: currentState.seenNestedCallIds.size,
        maxNestedCalls: MAX_MCP_CALLS,
        bridgeCalls: currentState.seenBridgeCallIds.size,
        maxBridgeCalls: MAX_BRIDGE_CALLS,
        directAuditRecords: currentState.directAudit.length
      } : {},
      counts: currentState ? {
        events: Object.values(currentState.eventCounts).reduce((sum, value) => sum + value, 0),
        incomingBytes: currentState.incomingBytes,
        bridges: currentState.seenBridgeCallIds.size,
        nestedMcp: currentState.seenNestedCallIds.size,
        directAudits: currentState.directAudit.length
      } : {},
      eventCounts: currentState ? { ...currentState.eventCounts, result: currentState.resultSeen ? 1 : 0 } : {},
      eventDigest: activityDigest,
      execBridge: currentState ? [...currentState.bridgeReceipt] : [],
      nestedMcp: currentState ? [...currentState.mcpReceipt] : [],
      directServerAudit: currentState ? {
        crosschecked: currentState.directAudit.length,
        records: currentState.directAudit.map(record => ({ ...record, arguments: { ...record.arguments }, result: { ...record.result } }))
      } : { crosschecked: 0, records: [] },
      outer: { content: outerDigest },
      cleanup: cloneJson(cleanupOutcome, 'cleanup receipt'),
      policyVerdict: currentState?.policyVerdict || { verdict: 'pending' }
    };
    cloneJson(receipt, 'receipt');
    return receipt;
  }

  function attachReceipt(error, currentState = state) {
    if (error instanceof Error && currentState) error.codexEventReceipt = safeReceipt(currentState);
    return error;
  }

  function rejectPending(error) {
    if (initializePending) {
      const pending = initializePending;
      initializePending = null;
      pending.reject(error);
    }
    if (queryPending) {
      const pending = queryPending;
      queryPending = null;
      pending.reject(error);
    }
  }

  function poison(error) {
    const governedError = error instanceof Error ? error : new Error(String(error));
    if (!poisoned) {
      poisoned = true;
      poisonError = governedError;
      if (state) state.policyVerdict = { verdict: 'deny', reason: errorMessage(governedError).slice(0, 160) };
      if (state?.quietReject) state.quietReject(governedError);
      rejectPending(governedError);
      void performCleanup();
    }
    return poisonError;
  }

  function armTimeout(kind, callback) {
    return setTimeout(() => callback(new Error(`Codex ${kind} timeout`)), bindings.codexMcpTimeout);
  }

  function appendStderr(data) {
    const chunk = Buffer.isBuffer(data) ? data : Buffer.from(String(data));
    const available = CODEX_STDERR_MAX_BYTES - stderr.length;
    if (available > 0) stderr = Buffer.concat([stderr, chunk.subarray(0, available)]);
  }

  function sendLine(message) {
    if (!codexProcess?.stdin || typeof codexProcess.stdin.write !== 'function') throw new Error('Codex stdin is unavailable');
    const line = `${JSON.stringify(message)}\n`;
    if (Buffer.byteLength(line, 'utf8') > CODEX_MAX_SERIALIZED_BYTES) throw new Error('Codex request exceeds serialized-byte bound');
    codexProcess.stdin.write(line);
  }

  function handleLine(line) {
    try {
      if (typeof line !== 'string' || Buffer.byteLength(line, 'utf8') > CODEX_MAX_SERIALIZED_BYTES) throw new Error('Codex stdout line exceeds serialized-byte bound');
      incomingBytes += Buffer.byteLength(line, 'utf8') + 1;
      if (incomingBytes > CODEX_MAX_INCOMING_BYTES) throw new Error('Codex cumulative incoming bytes exceed the bound');
      if (state) state.incomingBytes = incomingBytes;
      const message = JSON.parse(line);
      cloneJson(message, 'stdout message');
      if (!isObject(message) || message.jsonrpc !== '2.0') throw new Error('Codex stdout message is not JSON-RPC 2.0');
      if (message.method !== undefined) {
        if (message.method !== 'codex/event' || message.id !== undefined) throw new Error('Codex unknown method or notification');
        const params = validateJsonRpcEventEnvelope(message);
        if (!state || params._meta.requestId !== 2 || state.resultSeen || closed) throw new Error('Codex unknown or late event');
        validateEventState(state, params, serverName, governance.allowedSet);
        return;
      }
      if (message.id === 1) {
        if (!initializePending) throw new Error('Codex unexpected initialize response');
        const result = validateInitializeResult(message);
        initializeVersion = result.serverInfo.version;
        const pending = initializePending;
        initializePending = null;
        pending.resolve(result);
        return;
      }
      if (message.id === 2) {
        if (!queryPending || !state || state.resultSeen || !state.taskCompleteSeen) throw new Error('Codex unexpected or late tools/call result');
        if (!exactKeys(message, ['jsonrpc', 'id', 'result']) || message.jsonrpc !== '2.0' || message.id !== 2) {
          throw new Error('Codex tools/call result envelope is invalid');
        }
        const result = validateOuterResult(message.result, state);
        state.resultSeen = true;
        state.result = result;
        const pending = queryPending;
        queryPending = null;
        pending.resolve(result);
        return;
      }
      throw new Error('Codex unknown JSON-RPC id');
    } catch (error) {
      poison(new Error(`Codex protocol failure: ${errorMessage(error).slice(0, 240)}`));
    }
  }

  function waitForChildExit(timeoutMs) {
    if (!codexProcess || childExited) return Promise.resolve(childExited || !codexProcess);
    return new Promise(resolvePromise => {
      const timer = setTimeout(() => resolvePromise(false), timeoutMs);
      const check = () => {
        clearTimeout(timer);
        resolvePromise(childExited);
      };
      codexProcess.once?.('exit', check);
      codexProcess.once?.('close', check);
    });
  }

  function boundedMcpStop() {
    if (!mcpServer) return Promise.resolve();
    return new Promise((resolvePromise, rejectPromise) => {
      let settled = false;
      const timer = setTimeout(() => {
        if (settled) return;
        settled = true;
        rejectPromise(new Error('MCP server stop timeout'));
      }, CODEX_CLEANUP_TIMEOUT_MS);
      Promise.resolve().then(() => mcpServer.stop()).then(value => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolvePromise(value);
      }, error => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        rejectPromise(error);
      });
    });
  }

  function updateLateCleanupOutcome(outcome) {
    cleanupOutcome = {
      ...cleanupOutcome,
      lateMcpStop: cloneJson(outcome, 'late MCP cleanup outcome'),
      ...(outcome.status === 'failed' ? {
        status: 'failed',
        errors: [...(cleanupOutcome.errors || []), outcome.error].filter(Boolean).slice(0, 4)
      } : {})
    };
  }

  function scheduleLateMcpCleanup() {
    if (mcpLateStopPromise) return mcpLateStopPromise;
    mcpLateStopPromise = mcpStartPromise.then(async () => {
      mcpStartResolved = true;
      if (!mcpStartTimedOut) {
        const outcome = { status: 'not_needed' };
        updateLateCleanupOutcome(outcome);
        return outcome;
      }
      try {
        // The first cleanup may already have stopped the pre-listen server.
        // Once start resolves, stop again to close the listener it created.
        await performCleanup();
        await boundedMcpStop();
        const outcome = { status: 'succeeded' };
        updateLateCleanupOutcome(outcome);
        return outcome;
      } catch (error) {
        const outcome = { status: 'failed', error: errorMessage(error).slice(0, 160) };
        updateLateCleanupOutcome(outcome);
        throw error;
      }
    }, error => {
      mcpStartResolved = true;
      const outcome = { status: 'start_failed', error: errorMessage(error).slice(0, 160) };
      updateLateCleanupOutcome(outcome);
      return outcome;
    });
    // Keep a late stop failure observable through the exposed promise while
    // also handling the rejection so a late listener cannot become an
    // unhandled rejection after startup has already failed.
    mcpLateStopPromise.catch(() => {});
    return mcpLateStopPromise;
  }

  function exposeLateCleanup(error) {
    if (error instanceof Error && mcpLateStopPromise) {
      Object.defineProperty(error, 'codexMcpLateCleanup', {
        configurable: true,
        value: mcpLateStopPromise
      });
    }
    return error;
  }

  async function performCleanup() {
    if (cleanupPromise) return cleanupPromise;
    cleanupPromise = (async () => {
      const errors = [];
      closed = true;
      if (initializeTimer) clearTimeout(initializeTimer);
      if (turnTimer) clearTimeout(turnTimer);
      initializeTimer = null;
      turnTimer = null;
      let escalated = false;
      let childTerminationRequested = false;
      if (codexProcess && !childExited && typeof codexProcess.kill === 'function') {
        childTerminationRequested = true;
        try { codexProcess.kill('SIGTERM'); } catch (error) { errors.push(errorMessage(error)); }
        if (!childExited && !(await waitForChildExit(CODEX_CLEANUP_TIMEOUT_MS))) {
          escalated = true;
          try { codexProcess.kill('SIGKILL'); } catch (error) { errors.push(errorMessage(error)); }
          if (!childExited && !(await waitForChildExit(CODEX_CLEANUP_TIMEOUT_MS))) errors.push('child exit timeout');
        }
      }
      let readerClosed = !reader;
      try {
        if (reader && typeof reader.close === 'function') {
          reader.close();
          readerClosed = true;
        }
      } catch (error) { errors.push(errorMessage(error)); }
      let mcpServerStopped = !mcpServer;
      if (mcpServer) {
        try {
          await boundedMcpStop();
          mcpServerStopped = true;
        } catch (error) { errors.push(errorMessage(error)); }
      }
      cleanupOutcome = {
        status: errors.length === 0 && readerClosed && mcpServerStopped && (!codexProcess || childExited) &&
          (!mcpStartTimedOut || mcpStartResolved) ? 'succeeded' : 'failed',
        readerClosed,
        mcpServerStopped,
        directChild: codexProcess ? { terminationRequested: childTerminationRequested, exited: childExited, exitCode: childExitCode, exitSignal: childExitSignal, escalated } : null,
        errors: errors.slice(0, 4),
        lateMcpStop: mcpStartTimedOut ? { status: 'pending' } : { status: 'not_needed' }
      };
      return cleanupOutcome;
    })();
    return cleanupPromise;
  }

  try {
    // Isolation and the exact Probe preflight above are intentionally complete
    // before this constructor or child-process spawn can have side effects.
    mcpServer = new BuiltInMCPServer(options.agent, {
      port: 0,
      host: '127.0.0.1',
      debug: options.debug,
      governed: true,
      serverName
    });
    mcpStartPromise = Promise.resolve().then(() => mcpServer.start());
    scheduleLateMcpCleanup();
    let startTimer;
    let started;
    try {
      const timeoutPromise = new Promise((_, reject) => {
        startTimer = setTimeout(() => {
          mcpStartTimedOut = true;
          reject(new Error('Codex MCP startup timeout'));
        }, bindings.codexMcpTimeout);
      });
      started = await Promise.race([mcpStartPromise, timeoutPromise]);
    } finally {
      if (startTimer) clearTimeout(startTimer);
    }
    if (!started || typeof started.host !== 'string' || !Number.isInteger(started.port)) throw new Error('Probe MCP startup result is invalid');
    const mcpUrl = `http://${started.host}:${started.port}/mcp`;

    codexProcess = spawn(executable.path, ['mcp-server'], {
      cwd: bindings.cwd,
      env: buildCodexEnvironment(isolation.codexHome),
      stdio: ['pipe', 'pipe', 'pipe']
    });
    if (!codexProcess?.stdin || !codexProcess?.stdout || !codexProcess?.stderr || typeof codexProcess.on !== 'function') {
      throw new Error('Codex direct child transport is unavailable');
    }
    reader = createInterface({ input: codexProcess.stdout, crlfDelay: Infinity });
    reader.on('line', handleLine);
    codexProcess.stderr.on('data', appendStderr);
    codexProcess.on('error', error => { if (!closed) poison(new Error(`Codex child error: ${errorMessage(error)}`)); });
    codexProcess.on('exit', (code, signal) => {
      childExited = true;
      childExitCode = code;
      childExitSignal = signal;
      if (!closed) poison(new Error(`Codex child exited: ${code ?? signal}`));
    });
    codexProcess.on('close', (code, signal) => {
      childExited = true;
      childExitCode ??= code;
      childExitSignal ??= signal;
      if (!closed) poison(new Error(`Codex child closed: ${code ?? signal}`));
    });

    initializePending = {};
    const initializePromise = new Promise((resolvePromise, rejectPromise) => {
      initializePending.resolve = resolvePromise;
      initializePending.reject = rejectPromise;
    });
    initializePromise.catch(() => {});
    sendLine({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: INITIALIZE_PROTOCOL_VERSION,
        capabilities: { tools: {} },
        clientInfo: { ...INITIALIZE_CLIENT_INFO }
      }
    });
    initializeTimer = armTimeout('initialize', error => poison(error));
    try {
      await initializePromise;
    } finally {
      if (initializeTimer) clearTimeout(initializeTimer);
      initializeTimer = null;
    }

    const fullPrompt = combinePrompts(options.systemPrompt, options.customPrompt);
    const engine = {
      sessionId: session.id,
      session,
      async *query(prompt) {
        if (closed) throw new Error('Codex engine is closed');
        if (poisoned) throw attachReceipt(poisonError || new Error('Codex engine is poisoned'));
        if (queryReserved) throw new Error('Codex engine accepts exactly one query');
        queryReserved = true;
        if (typeof prompt !== 'string' || prompt.length === 0 || prompt.length > MAX_STRING_LENGTH) throw new Error('Codex query prompt is invalid');
        const queryPrompt = fullPrompt ? `${fullPrompt}\n\n${prompt}` : prompt;
        state = {
          bindings,
          codexHome: isolation.codexHome,
          serverName,
          mcpServer,
          prompt: queryPrompt,
          threadId: null,
          turnId: null,
          taskStartedAt: null,
          turnStartedAtUnixMs: null,
          lastProgressToken: 0,
          identity: null,
          startupComplete: false,
          startupUpdateCount: 0,
          startingSeen: false,
          readySeen: false,
          readyDynamic: false,
          taskStarted: false,
          taskCompleteSeen: false,
          incomingBytes,
          rawMessageRoles: [],
          rawAssistantSeen: false,
          rawReasoningIds: new Set(),
          reasoningItems: new Map(),
          completedReasoningIds: new Set(),
          userItem: null,
          userItemStarted: false,
          userItemCompleted: false,
          userMessageSeen: false,
          agentItemStarted: false,
          agentCompleted: false,
          agentItem: null,
          agentItemId: null,
          deltaText: '',
          agentMessage: null,
          agentMessageSeen: false,
          taskCompleteMessage: null,
          resultSeen: false,
          result: null,
          openItems: new Map(),
          seenItemIds: new Set(),
          mcp: new Map(),
          seenNestedCallIds: new Set(),
          bridge: new Map(),
          activeBridge: null,
          seenBridgeCallIds: new Set(),
          seenBridgeItemIds: new Set(),
          bridgeReceipt: [],
          mcpReceipt: [],
          directAudit: [],
          auditOrdinals: new Set(),
          auditConsumedCount: 0,
          lastAuditOrdinal: 0,
          toolSuccessCount: 0,
          tokenCount: 0,
          eventCounts: Object.create(null),
          activity: [],
          quietWindowArmed: false,
          policyVerdict: { verdict: 'pending' }
        };
        let quietTimer;
        try {
          const args = buildCodexInitialToolArgs({
            prompt: queryPrompt,
            model: bindings.model,
            thinkingEffort: bindings.thinkingEffort,
            cwd: bindings.cwd,
            sandbox: bindings.sandbox,
            approvalPolicy: bindings.approvalPolicy,
            mcpServerName: serverName,
            mcpServerUrl: mcpUrl
          });
          if (requestSent) throw new Error('Codex request latch was already used');
          requestSent = true;
          queryPending = {};
          const resultPromise = new Promise((resolvePromise, rejectPromise) => {
            queryPending.resolve = resolvePromise;
            queryPending.reject = rejectPromise;
          });
          resultPromise.catch(() => {});
          sendLine({ jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'codex', arguments: args } });
          turnTimer = armTimeout('turn', error => poison(error));
          const result = await resultPromise;
          if (turnTimer) clearTimeout(turnTimer);
          turnTimer = null;
          if (!state.taskCompleteSeen || !state.resultSeen || state.mcp.size !== 0 || state.openItems.size !== 0 || state.toolSuccessCount < 1) throw new Error('Codex governed lifecycle admission is incomplete');
          // The capture held the result for a 1.5 second quiet window.  Any
          // correlated traffic during this period poisons the turn.
          await new Promise((resolvePromise, rejectPromise) => {
            state.quietWindowArmed = true;
            quietTimer = setTimeout(resolvePromise, CODEX_QUIET_WINDOW_MS);
            state.quietReject = rejectPromise;
          });
          if (poisoned) throw poisonError;
          state.policyVerdict = { verdict: 'allow' };
          await performCleanup();
          if (cleanupOutcome.status !== 'succeeded') throw new Error('Codex cleanup did not admit success');
          session.setConversationId(state.threadId);
          session.incrementMessageCount();
          yield { type: 'text', content: result.content[0].text };
          yield {
            type: 'metadata',
            data: {
              sessionId: session.id,
              conversationId: session.conversationId,
              messageCount: session.messageCount,
              codexEventReceipt: safeReceipt(state)
            }
          };
        } catch (error) {
          if (quietTimer) clearTimeout(quietTimer);
          if (turnTimer) clearTimeout(turnTimer);
          turnTimer = null;
          const failure = error instanceof Error ? error : new Error(String(error));
          if (state) state.policyVerdict = { verdict: 'deny', reason: errorMessage(failure).slice(0, 160) };
          if (!poisoned) poison(failure);
          if (state?.quietReject) state.quietReject(poisonError || failure);
          await performCleanup();
          throw attachReceipt(poisonError || failure, state);
        }
      },
      getSession() { return session.getInfo(); },
      getTransportState() {
        return {
          poisoned,
          closed,
          stderr: stderr.toString('utf8'),
          stderrBytes: stderr.length,
          eventHandlers: state ? 1 : 0,
          pendingRequests: (initializePending ? 1 : 0) + (queryPending ? 1 : 0),
          quietWindowArmed: state?.quietWindowArmed === true,
          cleanup: cloneJson(cleanupOutcome, 'cleanup state')
        };
      },
      close() {
        if (!cleanupPromise) {
          const error = new Error('Codex engine closed');
          if (state) state.policyVerdict = { verdict: 'deny', reason: error.message };
          rejectPending(error);
          if (!poisoned) { poisoned = true; poisonError = error; }
          void performCleanup();
        }
        return cleanupPromise;
      }
    };
    return engine;
  } catch (error) {
    if (!poisoned) poison(error);
    await performCleanup();
    throw exposeLateCleanup(attachReceipt(error, state));
  }
}
