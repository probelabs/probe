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
import { dirname, isAbsolute, join, resolve } from 'path';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { Session } from '../shared/Session.js';

export const CODEX_MODEL = 'gpt-5.6-luna';
export const CODEX_REASONING_EFFORT = 'xhigh';
export const CODEX_SANDBOX = 'read-only';
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

const INITIALIZE_PROTOCOL_VERSION = '2024-11-05';
const INITIALIZE_CLIENT_INFO = Object.freeze({ name: 'protocol-capture-r4', version: '1.0.0' });
const PROBE_TOOL_PREFIX = 'mcp__probe__';
const PROBE_TOOLS = Object.freeze(['search', 'extract', 'listFiles']);
const PROBE_TOOL_SET = new Set(PROBE_TOOLS);
const EVENT_TYPES = new Set([
  'agent_message',
  'agent_message_content_delta',
  'item_completed',
  'item_started',
  'mcp_startup_complete',
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
const ALLOWED_ENVIRONMENT = Object.freeze(['PATH', 'TMPDIR', 'LANG', 'LC_ALL', 'LC_CTYPE']);

const MAX_STRING_LENGTH = 131072;
const MAX_ID_LENGTH = 512;
const MAX_EVENT_TYPE_LENGTH = 64;
const MAX_ACTIVITY = 256;

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
    mcp_servers: { [mcpServerName]: { url: mcpServerUrl } }
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
  if (!isAbsolute(executablePath || '') || !isAbsolute(expectedExecutablePath || '')) {
    throw new Error('Codex executable paths must be absolute');
  }
  if (typeof expectedExecutableSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(expectedExecutableSha256)) {
    throw new Error('Codex executable SHA-256 pin is invalid');
  }
  let canonicalExecutable;
  let canonicalExpected;
  try {
    canonicalExecutable = realpathSync(resolve(executablePath));
    canonicalExpected = realpathSync(resolve(expectedExecutablePath));
  } catch {
    throw new Error('Codex executable path cannot be canonicalized');
  }
  if (canonicalExecutable !== canonicalExpected) throw new Error('Codex executable path pin does not match');
  const sha256 = createHash('sha256').update(readFileSync(canonicalExecutable)).digest('hex');
  if (sha256 !== expectedExecutableSha256) throw new Error('Codex executable SHA-256 does not match');
  return { path: canonicalExecutable, sha256 };
}

function validateGovernedAgent(agent) {
  if (!agent || typeof agent !== 'object') {
    throw new Error('Codex Probe governance requires the real top-level ProbeAgent');
  }
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

function validateSessionIdentity(msg, bindings) {
  const keys = ['type', 'session_id', 'thread_id', 'model', 'model_provider_id', 'approval_policy',
    'approvals_reviewer', 'permission_profile', 'reasoning_effort', 'rollout_path', 'cwd'];
  const optional = ['active_permission_profile'];
  if (!isObject(msg) || !Object.keys(msg).every(key => keys.includes(key) || optional.includes(key)) ||
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
  if (!isAbsolute(identity.rollout_path) || identity.model !== bindings.model || identity.model_provider_id !== 'openai' ||
      identity.approval_policy !== bindings.approvalPolicy || identity.approvals_reviewer !== 'user' ||
      identity.reasoning_effort !== bindings.thinkingEffort || identity.cwd !== bindings.cwd) {
    throw new Error('Codex session_configured identity does not match requested bindings');
  }
  if (identity.session_id !== identity.thread_id) throw new Error('Codex session/thread identity is not fresh and paired');
  if (Object.prototype.hasOwnProperty.call(msg, 'active_permission_profile')) {
    if (!exactKeys(msg.active_permission_profile, ['id']) || msg.active_permission_profile.id !== ':read-only') {
      throw new Error('Codex active_permission_profile is not the pinned read-only representation');
    }
    identity.active_permission_profile = cloneJson(msg.active_permission_profile, 'active_permission_profile');
  }
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
    return { kind: 'assistant', id: item.id, text: part.text };
  }
  throw new Error('Codex raw_response_item role is not observed');
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
  return requireString(msg.turn_id, 'task_started.turn_id', MAX_ID_LENGTH);
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
  if (!exactKeys(result, ['content', 'structuredContent']) || !Array.isArray(result.content) || result.content.length !== 1 ||
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
  if (state.resultSeen) throw new Error('Codex traffic arrived after result');
  if (type !== 'session_configured' && params._meta.threadId !== state.threadId) throw new Error('Codex event thread_id mismatch');
  if (type === 'session_configured' || type === 'mcp_startup_complete') {
    if (params.id !== '') throw new Error('Codex pre-turn event id must be empty');
  } else if (type === 'task_started') {
    if (params.id !== params.msg.turn_id) throw new Error('Codex task_started id does not equal its turn_id');
  } else if (state.turnId === null || params.id !== state.turnId) {
    throw new Error('Codex event id does not equal the real turn_id');
  }
  if (!EVENT_TYPES.has(type)) throw new Error(`Codex event type ${type} is denied`);
  state.eventCounts[type] = (state.eventCounts[type] || 0) + 1;
  if (Object.values(state.eventCounts).reduce((sum, value) => sum + value, 0) > CODEX_MAX_EVENT_COUNT) {
    throw new Error('Codex event count exceeds the bound');
  }

  if (type === 'session_configured') {
    if (state.step !== 0 || state.identity) throw new Error('Codex session_configured is duplicated or out of order');
    state.identity = validateSessionIdentity(params.msg, state.bindings);
    state.threadId = identityThread(state.identity, params._meta.threadId);
    state.step = 1;
    return;
  }
  if (!state.identity) throw new Error('Codex event preceded session_configured');
  if (type === 'mcp_startup_complete') {
    if (state.step !== 1 || state.startupSeen || !exactKeys(params.msg, ['cancelled', 'failed', 'ready', 'type']) ||
        params.msg.type !== type || !Array.isArray(params.msg.ready) || !sameJson(params.msg.ready, [serverName]) ||
        !Array.isArray(params.msg.failed) || params.msg.failed.length !== 0 || !Array.isArray(params.msg.cancelled) || params.msg.cancelled.length !== 0) {
      throw new Error('Codex mcp_startup_complete arrays are not exact');
    }
    state.startupSeen = true;
    state.step = 2;
    return;
  }
  if (type === 'task_started') {
    if (state.step !== 2 || state.turnId !== null) throw new Error('Codex task_started is duplicated or out of order');
    state.turnId = validateTaskStarted(params.msg);
    if (params._meta.threadId !== state.threadId) throw new Error('Codex task_started thread mismatch');
    state.step = 3;
    return;
  }
  if (state.step < 3) throw new Error('Codex event preceded task_started');

  // MCP calls are the only events allowed to interrupt the captured normal
  // lifecycle.  Their fields are validated in full and balanced by call_id.
  if (type === 'mcp_tool_call_begin') {
    if (!exactKeys(params.msg, ['call_id', 'invocation', 'type'])) throw new Error('Codex MCP begin shape is invalid');
    const callId = requireString(params.msg.call_id, 'MCP begin call_id', MAX_ID_LENGTH);
    if (state.mcp.has(callId)) throw new Error('Codex MCP call_id was reused');
    const invocation = validateMcpInvocation(params.msg.invocation, serverName, allowedSet);
    state.mcp.set(callId, invocation);
    state.activity.push({ kind: type, callId, server: invocation.server, tool: invocation.tool });
    return;
  }
  if (type === 'mcp_tool_call_end') {
    if (!exactKeys(params.msg, ['call_id', 'duration_ms', 'invocation', 'result', 'type'])) throw new Error('Codex MCP end shape is invalid');
    const callId = requireString(params.msg.call_id, 'MCP end call_id', MAX_ID_LENGTH);
    const begin = state.mcp.get(callId);
    if (!begin) throw new Error('Codex MCP end has no matching begin');
    const invocation = validateMcpInvocation(params.msg.invocation, serverName, allowedSet);
    if (!sameJson(begin, invocation)) throw new Error('Codex MCP begin/end invocation differs');
    requireNumber(params.msg.duration_ms, 'MCP duration_ms', { integer: true, min: 0 });
    validateMcpResult(params.msg.result);
    state.mcp.delete(callId);
    state.activity.push({ kind: type, callId, server: invocation.server, tool: invocation.tool });
    return;
  }

  if (state.step === 3 && type === 'raw_response_item') {
    if (!exactKeys(params.msg, ['item', 'type']) || params.msg.type !== type) throw new Error('Codex raw_response_item wrapper is invalid');
    const parsed = validateRawMessageItem(state, params.msg.item);
    if (parsed.kind === 'developer' && state.rawInputCount === 0) state.rawInputCount++;
    else if (parsed.kind === 'user' && state.rawInputCount >= 1 && state.rawInputCount <= 2) state.rawInputCount++;
    else throw new Error('Codex raw input message order is not captured');
    if (state.rawInputCount === 3) state.step = 4;
    return;
  }
  if (state.step === 4 && type === 'item_started') {
    if (!exactKeys(params.msg, ['item', 'started_at_ms', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId) throw new Error('Codex UserMessage start wrapper is invalid');
    requireNumber(params.msg.started_at_ms, 'UserMessage started_at_ms', { integer: true, min: 0 });
    state.userItem = validateLifecycleItem(state, params.msg.item, 'UserMessage');
    state.step = 5;
    return;
  }
  if (state.step === 5 && type === 'item_completed') {
    if (!exactKeys(params.msg, ['completed_at_ms', 'item', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId ||
        !sameJson(params.msg.item, state.userItem)) throw new Error('Codex UserMessage completion is not paired');
    requireNumber(params.msg.completed_at_ms, 'UserMessage completed_at_ms', { integer: true, min: 0 });
    state.step = 6;
    return;
  }
  if (state.step === 6 && type === 'user_message') {
    validateUserMessage(params.msg, state);
    state.step = 7;
    return;
  }
  if (state.step === 7 && type === 'item_started') {
    if (!exactKeys(params.msg, ['item', 'started_at_ms', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId) throw new Error('Codex AgentMessage start wrapper is invalid');
    requireNumber(params.msg.started_at_ms, 'AgentMessage started_at_ms', { integer: true, min: 0 });
    state.agentItem = validateLifecycleItem(state, params.msg.item, 'AgentMessage');
    state.agentItemId = params.msg.item.id;
    state.step = 8;
    return;
  }
  if (state.step === 8 && type === 'agent_message_content_delta') {
    if (!exactKeys(params.msg, ['delta', 'item_id', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.item_id !== state.agentItemId || params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId) {
      throw new Error('Codex agent_message_content_delta binding is invalid');
    }
    state.deltaText += requireString(params.msg.delta, 'agent_message_content_delta.delta');
    if (state.deltaText.length > MAX_STRING_LENGTH) throw new Error('Codex response exceeds the bound');
    return;
  }
  if (state.step === 8 && type === 'item_completed') {
    if (!exactKeys(params.msg, ['completed_at_ms', 'item', 'thread_id', 'turn_id', 'type']) || params.msg.type !== type ||
        params.msg.thread_id !== state.threadId || params.msg.turn_id !== state.turnId ||
        !exactKeys(params.msg.item, ['content', 'id', 'phase', 'type']) || params.msg.item.id !== state.agentItemId ||
        params.msg.item.type !== 'AgentMessage' || params.msg.item.phase !== 'final_answer' || !Array.isArray(params.msg.item.content) ||
        params.msg.item.content.length !== 1 || !exactKeys(params.msg.item.content[0], ['text', 'type']) ||
        params.msg.item.content[0].type !== 'Text' || params.msg.item.content[0].text !== state.deltaText) {
      throw new Error('Codex AgentMessage completion is invalid');
    }
    requireNumber(params.msg.completed_at_ms, 'AgentMessage completed_at_ms', { integer: true, min: 0 });
    state.step = 9;
    return;
  }
  if (state.step === 9 && type === 'agent_message') {
    validateAgentMessage(params.msg, state);
    state.step = 10;
    return;
  }
  if (state.step === 10 && type === 'raw_response_item') {
    if (!exactKeys(params.msg, ['item', 'type']) || params.msg.type !== type) throw new Error('Codex raw assistant wrapper is invalid');
    const parsed = validateRawMessageItem(state, params.msg.item);
    if (parsed.kind !== 'assistant' || parsed.text !== state.agentMessage || parsed.id !== state.agentItemId) {
      throw new Error('Codex raw assistant message is not bound to the lifecycle item');
    }
    state.step = 11;
    return;
  }
  if (state.step === 11 && type === 'token_count') {
    validateTokenCount(params.msg);
    state.step = 12;
    return;
  }
  if (state.step === 12 && type === 'task_complete') {
    if (state.mcp.size !== 0) throw new Error('Codex task_complete arrived with unbalanced MCP calls');
    validateTaskComplete(params.msg, state);
    state.taskCompleteMessage = params.msg.last_agent_message;
    state.step = 13;
    return;
  }
  throw new Error(`Codex ${type} event is out of the captured order`);
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
  let poisoned = false;
  let poisonError = null;
  let closed = false;
  let cleanupPromise = null;
  let cleanupOutcome = { status: 'not_started' };
  let childExited = false;
  let childExitCode = null;
  let childExitSignal = null;
  let stderr = Buffer.alloc(0);

  const requested = buildCodexRequestedMetadata(bindings);

  function safeReceipt(currentState = state) {
    const receipt = {
      requested,
      effective: currentState?.identity ? cloneJson(currentState.identity, 'effective identity') : null,
      isolation: cloneJson(isolation, 'isolation receipt'),
      executable: { path: executable.path, sha256: executable.sha256 },
      initialize: { serverInfoVersion: initializeVersion },
      eventCounts: currentState ? { ...currentState.eventCounts, result: currentState.resultSeen ? 1 : 0 } : {},
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
        if (!queryPending || !state || state.resultSeen) throw new Error('Codex unexpected or late tools/call result');
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
    if (!codexProcess || childExited) return Promise.resolve(true);
    return new Promise(resolvePromise => {
      const timer = setTimeout(() => resolvePromise(childExited), timeoutMs);
      const check = () => {
        clearTimeout(timer);
        resolvePromise(true);
      };
      codexProcess.once?.('exit', check);
      codexProcess.once?.('close', check);
    });
  }

  async function performCleanup() {
    if (cleanupPromise) return cleanupPromise;
    cleanupPromise = (async () => {
      const errors = [];
      closed = true;
      try { if (reader && typeof reader.close === 'function') reader.close(); } catch (error) { errors.push(errorMessage(error)); }
      try { if (mcpServer) await mcpServer.stop(); } catch (error) { errors.push(errorMessage(error)); }
      let escalated = false;
      if (codexProcess && !childExited && typeof codexProcess.kill === 'function') {
        try { codexProcess.kill('SIGTERM'); } catch (error) { errors.push(errorMessage(error)); }
        if (!childExited && !(await waitForChildExit(CODEX_CLEANUP_TIMEOUT_MS))) {
          escalated = true;
          try { codexProcess.kill('SIGKILL'); } catch (error) { errors.push(errorMessage(error)); }
          if (!childExited && !(await waitForChildExit(CODEX_CLEANUP_TIMEOUT_MS))) errors.push('child exit timeout');
        }
      }
      cleanupOutcome = {
        status: errors.length === 0 ? 'succeeded' : 'failed',
        readerClosed: !!reader,
        mcpServerStopped: !!mcpServer,
        directChild: codexProcess ? { signal: 'SIGTERM', exited: childExited, exitCode: childExitCode, exitSignal: childExitSignal, escalated } : null,
        errors: errors.slice(0, 4)
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
    const started = await mcpServer.start();
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
    await initializePromise;

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
          prompt: queryPrompt,
          threadId: null,
          turnId: null,
          identity: null,
          startupSeen: false,
          step: 0,
          rawInputCount: 0,
          userItem: null,
          agentItem: null,
          agentItemId: null,
          deltaText: '',
          agentMessage: null,
          taskCompleteMessage: null,
          resultSeen: false,
          result: null,
          mcp: new Map(),
          eventCounts: Object.create(null),
          activity: [],
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
          const result = await resultPromise;
          if (state.step !== 13 || !state.resultSeen || state.mcp.size !== 0) throw new Error('Codex captured lifecycle admission is incomplete');
          // The capture held the result for a 1.5 second quiet window.  Any
          // correlated traffic during this period poisons the turn.
          await new Promise((resolvePromise, rejectPromise) => {
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
    throw attachReceipt(error, state);
  }
}
