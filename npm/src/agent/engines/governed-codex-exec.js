/**
 * Single-query Codex `exec --json` transport for the governed profile.
 *
 * This module intentionally has a smaller boundary than the MCP-server
 * engine. It retains only the final agent answer, bounded usage, and public
 * process facts. Raw JSONL events, stderr, reasoning, rollouts, and config
 * contents never cross the returned result boundary.
 */

import { spawn } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { mkdtempSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { StringDecoder } from 'node:string_decoder';
import { isAbsolute, normalize } from 'node:path';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { governSpawnedProcess } from '../processSupervisor.js';
import { governedCodexDispatch } from './codex.js';
import { buildGovernedCodexInitialToolArgs, validateGovernedCodexProfile } from './governed-codex-profile.js';
import { GovernedAnswerFailure, governedAnswerFailure, normalizeGovernedAnswerFailure } from './governed-answer-failure.js';

export const GOVERNED_CODEX_EXEC_PROTOCOL = 'probe.governed-codex-exec/v1';
export const GOVERNED_CODEX_EXEC_ATTESTATION_VERSION = 'probe.governed-codex-exec-attestation/v1';
export const GOVERNED_CODEX_EXEC_TRANSPORT = 'exec-jsonl-default-auth-v1';

const DEFAULT_TIMEOUT_MS = 600000;
const MIN_TIMEOUT_MS = 10;
const MAX_TIMEOUT_MS = 3600000;
const STDOUT_BYTE_CAP = 4 * 1024 * 1024;
const STDERR_BYTE_CAP = 1024 * 1024;
const JSONL_LINE_BYTE_CAP = 1024 * 1024;
const MAX_EVENTS = 256;
const MAX_TEXT_BYTES = 131072;
const MAX_EFFECTIVE_INPUT_BYTES = MAX_TEXT_BYTES;
const SAFE_ID = /^[A-Za-z0-9._:-]{1,128}$/;
const GOVERNED_CODEX_EXEC_FAILURE_CODES = new Set([
  'ANSWER_CARDINALITY', 'CANCELLED', 'CANONICAL', 'CLEANUP', 'CONFIG', 'DUPLICATE', 'EVENT',
  'EVENT_CATEGORY', 'EVENT_LIMIT', 'EVENT_ORDER', 'INCOMPLETE', 'INCOMPLETE_ITEM', 'ITEM',
  'ITEM_ORDER', 'ITEM_STATUS', 'JSONL', 'MCP', 'MCP_EVIDENCE', 'ONE_QUERY', 'OUTPUT_OVERFLOW',
  'SETUP', 'SPAWN', 'TIMEOUT', 'TOOL_POLICY', 'USAGE', 'VERSION', 'EXIT',
].map(code => `GOVERNED_CODEX_EXEC_${code}`));
const GOVERNED_CODEX_EXEC_ITEM_PREDICATES = new Set([
  'item_keys', 'item_id', 'item_text', 'item_phase', 'item_summary', 'item_server',
  'item_command', 'item_aggregated_output', 'item_exit_code', 'item_changes', 'item_status', 'tool_id',
]);
const GOVERNED_CODEX_EXEC_ITEM_TYPES = new Set(['agent_message', 'reasoning', 'mcp_tool_call', 'command_execution', 'file_change']);
const GOVERNED_CODEX_EXEC_ITEM_EVENT_TYPES = new Set(['item.started', 'item.completed']);
const GOVERNED_CODEX_EXEC_SAFE_FIELD_NAME = /^[A-Za-z_][A-Za-z0-9_.-]{0,63}$/;
const GOVERNED_CODEX_EXEC_FIELD_TYPES = new Set(['null', 'array', 'object', 'string', 'number', 'boolean']);
const USAGE_KEYS = new Set([
  'input_tokens', 'cached_input_tokens', 'cache_write_input_tokens',
  'output_tokens', 'reasoning_output_tokens'
]);
const PUBLIC_ITEM_TYPES = new Set(['agent_message', 'reasoning', 'mcp_tool_call']);
const NATIVE_ITEM_TYPES = new Set(['command_execution', 'file_change']);
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const RAW_DIGEST = /^[0-9a-f]{64}$/;
const VERSION_TEXT = /^[\x20-\x7e]{1,256}$/;

function summarizeStderr(value) {
  if (typeof value !== 'string' || value.length === 0) return null;
  const bytes = Buffer.byteLength(value, 'utf8');
  const normalized = value.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, '').trim();
  const safeMessage = /access token could not be refreshed because your refresh token was revoked/i.test(normalized)
    ? 'access_token_refresh_revoked' : undefined;
  return Object.freeze({ source: 'codex-exec-stderr/v1', bytes, digest: sha256(value), ...(safeMessage ? { safeMessage } : {}) });
}

function fail(code, diagnostic = undefined, event = undefined) {
  const error = new Error(`GOVERNED_CODEX_EXEC_${code}`);
  error.code = `GOVERNED_CODEX_EXEC_${code}`;
  const stderr = summarizeStderr(diagnostic);
  if (stderr) Object.defineProperty(error, 'diagnostic', { value: stderr, enumerable: true });
  if (event !== undefined) Object.defineProperty(error, 'event', { value: event, enumerable: true });
  return error;
}

function ownDataValue(value, key) {
  if (!value || (typeof value !== 'object' && typeof value !== 'function')) return undefined;
  try {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    return descriptor && 'value' in descriptor ? descriptor.value : undefined;
  } catch { return undefined; }
}

function describeFieldType(descriptor) {
  if (!descriptor || !Object.prototype.hasOwnProperty.call(descriptor, 'value')) return 'object';
  const value = descriptor.value;
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  const type = typeof value;
  return GOVERNED_CODEX_EXEC_FIELD_TYPES.has(type) ? type : 'object';
}

function describeFields(value) {
  if (!value || (typeof value !== 'object' && typeof value !== 'function')) return Object.freeze([]);
  let keys;
  try { keys = Object.keys(value); } catch { return Object.freeze([]); }
  const fields = [];
  for (const key of keys.sort().slice(0, 32)) {
    let descriptor;
    try { descriptor = Object.getOwnPropertyDescriptor(value, key); } catch { continue; }
    if (!descriptor) continue;
    const rawName = key;
    const name = GOVERNED_CODEX_EXEC_SAFE_FIELD_NAME.test(rawName) ? rawName : '<unsafe>';
    const type = describeFieldType(descriptor);
    const field = { name, type };
    if (Object.prototype.hasOwnProperty.call(descriptor, 'value') && typeof descriptor.value === 'string') {
      field.size = Buffer.byteLength(descriptor.value, 'utf8');
    } else if (Object.prototype.hasOwnProperty.call(descriptor, 'value') && Array.isArray(descriptor.value)) {
      field.size = descriptor.value.length;
    }
    fields.push(field);
  }
  fields.sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  return Object.freeze(fields.map(field => Object.freeze(field)));
}

function rejectedItemEvent(event, predicate) {
  const item = ownDataValue(event, 'item');
  const itemType = ownDataValue(item, 'type');
  if (!GOVERNED_CODEX_EXEC_ITEM_PREDICATES.has(predicate) ||
      !GOVERNED_CODEX_EXEC_ITEM_EVENT_TYPES.has(ownDataValue(event, 'type')) ||
      !GOVERNED_CODEX_EXEC_ITEM_TYPES.has(itemType)) return undefined;
  return freeze({
    source: 'codex-exec-rejected-item/v1', predicate,
    eventType: ownDataValue(event, 'type'), itemType,
    eventFields: describeFields(event), itemFields: describeFields(item),
  });
}

function projectExecStderr(value) {
  if (!ownObject(value)) return undefined;
  const source = ownDataValue(value, 'source');
  const bytes = ownDataValue(value, 'bytes');
  const digest = ownDataValue(value, 'digest');
  if (source !== 'codex-exec-stderr/v1' || !Number.isSafeInteger(bytes) || bytes < 1 || bytes > STDERR_BYTE_CAP ||
      !DIGEST.test(digest)) return undefined;
  const safeMessage = ownDataValue(value, 'safeMessage');
  return Object.freeze({ source, bytes, digest, ...(safeMessage === 'access_token_refresh_revoked' ? { safeMessage } : {}) });
}

function projectExecItemFields(value) {
  if (!Array.isArray(value) || value.length > 32) return undefined;
  const fields = [];
  let previousName = null;
  for (const field of value) {
    if (!ownObject(field)) return undefined;
    const keys = Object.keys(field).sort();
    if (keys.join(',') !== 'name,type' && keys.join(',') !== 'name,size,type') return undefined;
    const name = ownDataValue(field, 'name');
    const type = ownDataValue(field, 'type');
    if (typeof name !== 'string' || (name !== '<unsafe>' && !GOVERNED_CODEX_EXEC_SAFE_FIELD_NAME.test(name)) ||
        !GOVERNED_CODEX_EXEC_FIELD_TYPES.has(type) || (previousName !== null && name < previousName)) return undefined;
    const hasSize = keys.includes('size');
    const size = ownDataValue(field, 'size');
    if (hasSize && (type !== 'string' && type !== 'array' || !Number.isSafeInteger(size) || size < 0)) return undefined;
    if (!hasSize && (type === 'string' || type === 'array')) return undefined;
    fields.push(Object.freeze({ name, type, ...(hasSize ? { size } : {}) }));
    previousName = name;
  }
  return Object.freeze(fields);
}

function projectExecItemEvent(value) {
  if (!ownObject(value) || Object.keys(value).sort().join(',') !== 'eventFields,eventType,itemFields,itemType,predicate,source') return undefined;
  const source = ownDataValue(value, 'source');
  const predicate = ownDataValue(value, 'predicate');
  const eventType = ownDataValue(value, 'eventType');
  const itemType = ownDataValue(value, 'itemType');
  const eventFields = projectExecItemFields(ownDataValue(value, 'eventFields'));
  const itemFields = projectExecItemFields(ownDataValue(value, 'itemFields'));
  if (source !== 'codex-exec-rejected-item/v1' || !GOVERNED_CODEX_EXEC_ITEM_PREDICATES.has(predicate) ||
      !GOVERNED_CODEX_EXEC_ITEM_EVENT_TYPES.has(eventType) || !GOVERNED_CODEX_EXEC_ITEM_TYPES.has(itemType) ||
      !eventFields || !itemFields) return undefined;
  return freeze({ source, predicate, eventType, itemType, eventFields, itemFields });
}

/** Project an exec error into the closed public diagnostic carried by Probe. */
export function projectGovernedCodexExecFailure(error) {
  const code = ownDataValue(error, 'code');
  if (typeof code !== 'string' || !GOVERNED_CODEX_EXEC_FAILURE_CODES.has(code)) return null;
  const stderr = projectExecStderr(ownDataValue(error, 'diagnostic'));
  const event = projectExecItemEvent(ownDataValue(error, 'event'));
  return Object.freeze({ version: 'probe.governed-codex-exec-failure/v1', code,
    ...(stderr ? { stderr } : {}), ...(event ? { event } : {}) });
}

/** Normalize only the explicit exec transport; legacy paths retain their old shape. */
export function normalizeGovernedCodexExecFailure(error, boundary) {
  if (error instanceof GovernedAnswerFailure) return error;
  const diagnostic = projectGovernedCodexExecFailure(error);
  if (!diagnostic) return normalizeGovernedAnswerFailure(error, 'provider_engine', null, null, null, null, null, boundary);
  return governedAnswerFailure('provider_engine', null, null, null, null, null, null, boundary, null, diagnostic);
}

function canonicalJson(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number' && Number.isFinite(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (ownObject(value)) return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  throw fail('CANONICAL');
}

function sha256(value) {
  const bytes = Buffer.isBuffer(value) || value instanceof Uint8Array
    ? Buffer.from(value) : typeof value === 'string' ? Buffer.from(value, 'utf8') : Buffer.from(canonicalJson(value), 'utf8');
  return `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
}

function digestText(value) { return sha256(value); }

function requireDigest(value, label) {
  if (typeof value !== 'string' || !DIGEST.test(value)) throw new TypeError(`Invalid ${label}`);
  return value;
}

function freeze(value) {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}

function ownObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value) &&
    (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
}

function exactKeys(value, keys) {
  if (!ownObject(value)) return false;
  const actual = Object.keys(value).sort();
  return actual.length === keys.length && keys.every(key => actual.includes(key));
}

function safeAbsoluteFile(value, label) {
  if (typeof value !== 'string' || !isAbsolute(value) || value.includes('\0') || normalize(value) !== value) {
    throw new TypeError(`Invalid ${label}`);
  }
  try {
    const resolved = realpathSync(value);
    if (!statSync(resolved).isFile()) throw new Error('not a file');
    return resolved;
  } catch {
    throw new TypeError(`Invalid ${label}`);
  }
}

function normalizeCliDigest(value) {
  if (typeof value !== 'string') throw new TypeError('Missing Codex executable SHA-256');
  const bare = /^[0-9a-f]{64}$/.test(value) ? `sha256:${value}` : value;
  return requireDigest(bare, 'Codex executable SHA-256');
}

function executableDigest(command) {
  try { return sha256(readFileSync(command)); } catch { throw new TypeError('Unable to read Codex executable'); }
}

function safeVersion(value) {
  if (typeof value !== 'string') throw fail('VERSION');
  const version = value.trim();
  if (!VERSION_TEXT.test(version) || version.includes('\n') || version.includes('\r')) throw fail('VERSION');
  return version;
}

function validateTimeout(value) {
  if (value === undefined) return DEFAULT_TIMEOUT_MS;
  if (!Number.isSafeInteger(value) || value < MIN_TIMEOUT_MS || value > MAX_TIMEOUT_MS) {
    throw new TypeError('Invalid governed Codex exec timeout');
  }
  return value;
}

function validateSignal(value) {
  if (value === undefined) return;
  if (!value || typeof value !== 'object' || typeof value.aborted !== 'boolean' ||
      typeof value.addEventListener !== 'function' || typeof value.removeEventListener !== 'function') {
    throw new TypeError('Invalid governed Codex exec signal');
  }
}

function tomlLiteral(value) {
  if (typeof value === 'string') return JSON.stringify(value);
  if (typeof value === 'boolean') return String(value);
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  if (Array.isArray(value)) {
    if (value.some(item => item !== null && typeof item === 'object')) throw fail('CONFIG');
    return `[${value.map(tomlLiteral).join(',')}]`;
  }
  throw fail('CONFIG');
}

function flattenConfig(value, prefix, output) {
  if (!ownObject(value)) throw fail('CONFIG');
  for (const key of Object.keys(value).sort()) {
    if (!/^[A-Za-z0-9_-]+$/.test(key)) throw fail('CONFIG');
    const path = prefix ? `${prefix}.${key}` : key;
    const child = value[key];
    if (ownObject(child)) flattenConfig(child, path, output);
    else output.push(`${path}=${tomlLiteral(child)}`);
  }
}

function cloneEnvironment() {
  const env = { ...process.env };
  // `exec --ignore-user-config` uses the normal auth location when this is
  // absent. Do not let a caller's isolated test/profile home leak into exec.
  delete env.CODEX_HOME;
  return env;
}

function bareDigest(value) {
  const bytes = typeof value === 'string' ? Buffer.from(value, 'utf8') : Buffer.from(canonicalJson(value), 'utf8');
  return createHash('sha256').update(bytes).digest('hex');
}

function profileProjection(profile) {
  const native = Array.isArray(profile.probeMcpTools);
  const base = {
    profileDigest: bareDigest(profile),
    cwdDigest: bareDigest(profile.cwd),
    ...(native ? {
      probeMcpToolsDigest: bareDigest(profile.probeMcpTools),
      codexNativeToolsDigest: bareDigest(profile.codexNativeTools),
      probeMcpTools: [...profile.probeMcpTools],
      codexNativeTools: [...profile.codexNativeTools],
    } : { probeToolsDigest: bareDigest(profile.probeTools) }),
    model: profile.model,
    reasoningEffort: profile.reasoningEffort,
    sandbox: profile.sandbox,
    approvalPolicy: profile.approvalPolicy,
  };
  return freeze(base);
}

function loopbackDigest(mcp, config) {
  const server = config?.mcp_servers?.[mcp.name];
  if (!ownObject(server) || server.url !== mcp.url || !Array.isArray(server.enabled_tools) ||
      server.default_tools_approval_mode !== 'prompt' || !ownObject(server.tools)) throw fail('MCP');
  const tools = server.enabled_tools.map(name => {
    if (typeof name !== 'string' || !ownObject(server.tools[name]) || server.tools[name].approval_mode !== 'approve') throw fail('MCP');
    return name;
  });
  return sha256({ version: 'probe.governed-codex-loopback/v1', name: mcp.name, url: mcp.url,
    enabledTools: tools, defaultToolsApprovalMode: server.default_tools_approval_mode,
    tools: Object.fromEntries(tools.sort().map(name => [name, { approvalMode: server.tools[name].approval_mode }])) });
}

function validatePrompt(prompt) {
  if (typeof prompt !== 'string' || Buffer.byteLength(prompt, 'utf8') < 1 ||
      Buffer.byteLength(prompt, 'utf8') > MAX_TEXT_BYTES || prompt.includes('\0')) {
    throw new TypeError('Invalid governed Codex exec prompt');
  }
  return prompt;
}

async function verifyCodexVersion(command, cwd, signal, timeoutMs) {
  let child;
  try {
    child = spawn(command, ['--version'], {
      cwd, env: cloneEnvironment(), stdio: ['ignore', 'pipe', 'pipe'], shell: false,
      detached: true, windowsHide: true,
    });
  } catch { throw fail('SPAWN'); }
  const handle = governSpawnedProcess(child, {
    captureStdout: true, stdoutByteCap: 4096, stderrByteCap: 4096,
    signalScope: 'process-group', executionTimeoutMs: Math.min(timeoutMs, 30000), signal,
  });
  const receipt = await handle.result;
  if (receipt.classification === 'aborted') throw fail('CANCELLED', receipt.stderr);
  if (receipt.classification === 'execution_timeout') throw fail('TIMEOUT', receipt.stderr);
  if (receipt.classification !== 'exited' || receipt.exitCode !== 0 || receipt.signal !== null ||
      !receipt.barriers.stdoutEOF || !receipt.barriers.stderrEOF || !receipt.barriers.close) throw fail('VERSION', receipt.stderr);
  return safeVersion(receipt.stdout);
}

/**
 * Build the no-shell launch description used by the governed exec engine.
 * The returned object deliberately does not expose the inherited environment.
 */
export function buildGovernedCodexExecLaunch({ codexPath, codexSha256, cliVersion, profile, prompt, mcp, modelInstructionsPath, modelInstructionsDigest }) {
  const command = safeAbsoluteFile(codexPath, 'Codex executable');
  const cliSha256 = normalizeCliDigest(codexSha256);
  const normalizedProfile = validateGovernedCodexProfile(profile);
  const normalizedPrompt = validatePrompt(prompt);
  if (!mcp || typeof mcp !== 'object' || typeof mcp.name !== 'string' || typeof mcp.url !== 'string') {
    throw new TypeError('Invalid governed Codex exec MCP binding');
  }
  const initial = buildGovernedCodexInitialToolArgs({ profile: normalizedProfile, prompt: normalizedPrompt, mcp });
  let effectiveConfig = initial.config;
  let effectiveInstructionsDigest = modelInstructionsDigest;
  let instructionsBytes = 0;
  if (modelInstructionsPath === undefined && modelInstructionsDigest !== undefined) {
    throw new TypeError('Model instructions digest requires a file');
  }
  if (modelInstructionsPath !== undefined) {
    const instructionsPath = safeAbsoluteFile(modelInstructionsPath, 'model instructions file');
    const instructions = readFileSync(instructionsPath);
    instructionsBytes = instructions.length;
    const actualInstructionsDigest = sha256(instructions);
    if (effectiveInstructionsDigest !== undefined && effectiveInstructionsDigest !== actualInstructionsDigest) {
      throw new TypeError('Model instructions digest mismatch');
    }
    effectiveInstructionsDigest = actualInstructionsDigest;
    requireDigest(effectiveInstructionsDigest, 'model instructions digest');
    effectiveConfig = freeze({ ...initial.config, model_instructions_file: instructionsPath });
  }
  const config = [];
  flattenConfig(effectiveConfig, '', config);
  // The MCP-server engine carries this as a request field. `exec` receives it
  // through its normal TOML config surface instead.
  config.push(`approval_policy=${tomlLiteral(normalizedProfile.approvalPolicy)}`);
  const args = [
    'exec', '--ignore-user-config', '--ignore-rules', '--ephemeral', '--json',
    '--model', normalizedProfile.model, '--sandbox', normalizedProfile.sandbox,
    '--cd', normalizedProfile.cwd,
    ...config.flatMap(entry => ['-c', entry]),
    normalizedPrompt,
  ];
  const dispatch = effectiveExecDispatch(normalizedPrompt, effectiveInstructionsDigest ?? null, instructionsBytes);
  const promptDigest = dispatch.promptDigest;
  const promptBytes = dispatch.promptBytes;
  const configDigest = sha256(effectiveConfig);
  const launchDigest = sha256({ version: 'probe.governed-codex-exec-launch/v1',
    cliPath: command, cliSha256, ...(cliVersion === undefined ? {} : { cliVersion: safeVersion(cliVersion) }),
    args: args.slice(0, -1), config: effectiveConfig, cwd: normalizedProfile.cwd,
    promptDigest, promptBytes,
    environmentPolicy: 'inherit-with-CODEX_HOME-omitted-v1',
    ...(effectiveInstructionsDigest === undefined ? {} : { instructionsDigest: effectiveInstructionsDigest, instructionsBytes }) });
  return Object.freeze({
    command,
    cliSha256,
    ...(cliVersion === undefined ? {} : { cliVersion: safeVersion(cliVersion) }),
    args: Object.freeze(args),
    argsWithoutPrompt: Object.freeze(args.slice(0, -1)),
    config: effectiveConfig,
    profile: normalizedProfile,
    requested: profileProjection(normalizedProfile),
    configDigest,
    launchDigest,
    promptDigest,
    promptBytes,
    ...(effectiveInstructionsDigest === undefined ? {} : { instructionsBytes }),
    ...(effectiveInstructionsDigest === undefined ? {} : { instructionsDigest: effectiveInstructionsDigest }),
    cwd: normalizedProfile.cwd,
    model: normalizedProfile.model,
    sandbox: normalizedProfile.sandbox,
    approvalPolicy: normalizedProfile.approvalPolicy,
    mcpName: mcp.name,
    mcpUrl: mcp.url,
    loopbackMcpDigest: loopbackDigest({ name: mcp.name, url: mcp.url }, effectiveConfig),
  });
}

function validateUsedToolItems(value) {
  if (!Array.isArray(value) || value.length > MAX_EVENTS) throw new TypeError('Invalid exec used tool evidence');
  const result = value.map(item => {
    if (!exactKeys(item, ['category', 'name', 'status', 'count']) ||
        !['mcp_tool_call', 'command_execution', 'file_change'].includes(item.category) ||
        item.status !== 'completed' || !Number.isSafeInteger(item.count) || item.count < 1 || item.count > MAX_EVENTS ||
        (item.category === 'mcp_tool_call' ? typeof item.name !== 'string' : item.name !== null)) {
      throw new TypeError('Invalid exec used tool evidence');
    }
    if (item.name !== null && (!/^[A-Za-z0-9_.:-]{1,128}$/.test(item.name))) throw new TypeError('Invalid exec used tool evidence');
    return { category: item.category, name: item.name, status: 'completed', count: item.count };
  });
  const keys = result.map(item => `${item.category}\u0000${item.name ?? ''}`);
  if (new Set(keys).size !== keys.length || keys.some((key, index) => index > 0 && key < keys[index - 1])) throw new TypeError('Invalid exec used tool evidence');
  return result;
}

function validateProfileProjection(value, profile) {
  const expected = profileProjection(profile);
  if (!ownObject(value) || canonicalJson(value) !== canonicalJson(expected)) throw new TypeError('Invalid exec requested profile projection');
  return expected;
}

function validateObserved(value, profileId = null) {
  if (!ownObject(value)) throw new TypeError('Invalid exec observed evidence');
  const keys = ['source', 'threadDigest', 'streamDigest', 'terminal', 'eventCount', 'completedItemCount', 'agentMessageCount', 'usedToolItems', 'probeMcpCallCount', 'finalDigest', 'finalBytes', 'processExitCode', 'processSignal'];
  if (!exactKeys(value, keys) || value.source !== 'codex-exec-jsonl/v1' || value.terminal !== 'turn.completed' ||
      value.processExitCode !== 0 || value.processSignal !== null ||
      !DIGEST.test(value.threadDigest) || !DIGEST.test(value.streamDigest) || !DIGEST.test(value.finalDigest) ||
      !Number.isSafeInteger(value.eventCount) || value.eventCount < 3 || value.eventCount > MAX_EVENTS ||
      !Number.isSafeInteger(value.completedItemCount) || value.completedItemCount < 1 || value.completedItemCount > MAX_EVENTS ||
      !Number.isSafeInteger(value.agentMessageCount) || value.agentMessageCount < 1 || value.agentMessageCount > value.completedItemCount ||
      !Number.isSafeInteger(value.probeMcpCallCount) || value.probeMcpCallCount < 0 || value.probeMcpCallCount > MAX_EVENTS ||
      !Number.isSafeInteger(value.finalBytes) || value.finalBytes < 0 || value.finalBytes > MAX_TEXT_BYTES) throw new TypeError('Invalid exec observed evidence');
  const usedToolItems = validateUsedToolItems(value.usedToolItems);
  const allowedNative = profileId === 'luna-xhigh-isolated-writer-v1' ? new Set(['command_execution', 'file_change'])
    : profileId === 'luna-xhigh-readonly-native-exec-v1' ? new Set(['command_execution']) : new Set();
  for (const item of usedToolItems) {
    if (item.category === 'mcp_tool_call' && !['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'].includes(item.name)) throw new TypeError('Invalid exec MCP tool');
    if (item.category !== 'mcp_tool_call' && !allowedNative.has(item.category)) throw new TypeError('Invalid exec native tool');
  }
  if (value.probeMcpCallCount !== usedToolItems.filter(item => item.category === 'mcp_tool_call').reduce((sum, item) => sum + item.count, 0)) throw new TypeError('Invalid exec MCP evidence');
  return { ...value, usedToolItems };
}

function validateUsageProjection(value) {
  if (!exactKeys(value, ['status', 'inputTokens', 'cachedInputTokens', 'outputTokens']) || value.status !== 'observed' ||
      !Number.isSafeInteger(value.inputTokens) || value.inputTokens < 0 ||
      !Number.isSafeInteger(value.cachedInputTokens) || value.cachedInputTokens < 0 ||
      !Number.isSafeInteger(value.outputTokens) || value.outputTokens < 0) throw new TypeError('Invalid exec usage');
  return { ...value };
}

/**
 * Format the single public receipt for the normal-auth governed exec path.
 * Raw prompts, event objects, stderr and environment values are intentionally
 * not accepted by this boundary.
 */
export function formatGovernedCodexExecAttestation(input = {}) {
  if (!ownObject(input)) throw new TypeError('Invalid exec attestation input');
  const profile = validateGovernedCodexProfile(input.profile);
  const requested = profileProjection(profile);
  const required = ['profile', 'cliPath', 'cliSha256', 'cliVersion', 'configDigest', 'launchDigest', 'cwdDigest', 'loopbackMcpDigest', 'promptDigest', 'promptBytes', 'observed', 'usage'];
  if (required.some(key => !Object.prototype.hasOwnProperty.call(input, key))) throw new TypeError('Invalid exec attestation input');
  const cliPath = safeAbsoluteFile(input.cliPath, 'Codex executable');
  const cliSha256 = normalizeCliDigest(input.cliSha256);
  const actualCliSha256 = executableDigest(cliPath);
  if (actualCliSha256 !== cliSha256) throw new TypeError('Codex executable SHA-256 mismatch');
  const cliVersion = safeVersion(input.cliVersion);
  const observed = validateObserved(input.observed, profile.profileId);
  const usage = validateUsageProjection(input.usage);
  if (!DIGEST.test(input.configDigest) || !DIGEST.test(input.launchDigest) || !RAW_DIGEST.test(input.cwdDigest) ||
      !DIGEST.test(input.loopbackMcpDigest) || !DIGEST.test(input.promptDigest) ||
      !Number.isSafeInteger(input.promptBytes) || input.promptBytes < 1 || input.promptBytes > MAX_TEXT_BYTES) throw new TypeError('Invalid exec attestation digest');
  if (input.cwdDigest !== requested.cwdDigest) throw new TypeError('Invalid exec attestation cwd binding');
  const enforced = {
    source: 'probe-host-codex-exec-argv/v1', transport: GOVERNED_CODEX_EXEC_TRANSPORT,
    cliPath, cliSha256, cliVersion, configDigest: input.configDigest, launchDigest: input.launchDigest,
    cwdDigest: input.cwdDigest, codexHome: 'omitted', environmentPolicy: 'inherit-with-CODEX_HOME-omitted-v1',
    ignoreUserConfig: true, ignoreRules: true, ephemeral: true, noShell: true,
    loopbackMcpDigest: input.loopbackMcpDigest,
  };
  const receipt = {
    version: GOVERNED_CODEX_EXEC_ATTESTATION_VERSION,
    profileId: profile.profileId,
    requested, enforced,
    observed, ...(input.invocationDigest === undefined ? {} : {
      executionContext: { source: 'caller', invocationDigest: requireDigest(input.invocationDigest, 'invocation digest') },
    }),
    dispatch: { source: 'probe-host-exec', tool: 'codex-exec', promptDigest: input.promptDigest, promptBytes: input.promptBytes },
    evidence: { eventCount: observed.eventCount, completedItemCount: observed.completedItemCount,
      agentMessageCount: observed.agentMessageCount, probeMcpCallCount: observed.probeMcpCallCount },
    usage,
  };
  return freeze(receipt);
}

export function validateGovernedCodexExecAttestation(input) {
  const baseKeys = ['dispatch', 'enforced', 'evidence', 'observed', 'profileId', 'requested', 'usage', 'version'];
  if (!ownObject(input) || !exactKeys(input, baseKeys) && !exactKeys(input, [...baseKeys, 'executionContext']) ||
      input.version !== GOVERNED_CODEX_EXEC_ATTESTATION_VERSION ||
      !['luna-xhigh-readonly-v1', 'luna-xhigh-readonly-native-exec-v1', 'luna-xhigh-isolated-writer-v1'].includes(input.profileId) ||
      !ownObject(input.enforced) || !ownObject(input.requested) || !ownObject(input.observed) ||
      !ownObject(input.dispatch) || !ownObject(input.evidence) || !ownObject(input.usage)) {
    throw new TypeError('Invalid governed Codex exec attestation');
  }
  const requestedKeys = input.profileId === 'luna-xhigh-readonly-v1'
    ? ['approvalPolicy', 'cwdDigest', 'model', 'probeToolsDigest', 'profileDigest', 'reasoningEffort', 'sandbox']
    : ['approvalPolicy', 'codexNativeTools', 'codexNativeToolsDigest', 'cwdDigest', 'model', 'probeMcpTools', 'probeMcpToolsDigest', 'profileDigest', 'reasoningEffort', 'sandbox'];
  if (!exactKeys(input.requested, requestedKeys) || input.requested.model !== 'gpt-5.6-luna' || input.requested.reasoningEffort !== 'xhigh' ||
      input.requested.approvalPolicy !== 'never' || !RAW_DIGEST.test(input.requested.profileDigest) || !RAW_DIGEST.test(input.requested.cwdDigest) ||
      (input.profileId === 'luna-xhigh-readonly-v1'
        ? (!RAW_DIGEST.test(input.requested.probeToolsDigest) || input.requested.sandbox !== 'read-only')
        : (!RAW_DIGEST.test(input.requested.probeMcpToolsDigest) || !RAW_DIGEST.test(input.requested.codexNativeToolsDigest) ||
          JSON.stringify(input.requested.probeMcpTools) !== JSON.stringify(['search', 'extract', 'listFiles']) ||
          JSON.stringify(input.requested.codexNativeTools) !== JSON.stringify(input.profileId === 'luna-xhigh-isolated-writer-v1' ? ['apply_patch', 'exec'] : ['exec']) ||
          input.requested.sandbox !== (input.profileId === 'luna-xhigh-isolated-writer-v1' ? 'workspace-write' : 'read-only')))) throw new TypeError('Invalid governed Codex exec requested profile');
  if (!exactKeys(input.enforced, ['cliPath', 'cliSha256', 'cliVersion', 'codexHome', 'configDigest', 'cwdDigest', 'environmentPolicy', 'ephemeral', 'ignoreRules', 'ignoreUserConfig', 'launchDigest', 'loopbackMcpDigest', 'noShell', 'source', 'transport']) ||
      !DIGEST.test(input.enforced.cliSha256) || !DIGEST.test(input.enforced.configDigest) || !DIGEST.test(input.enforced.launchDigest) ||
      !RAW_DIGEST.test(input.enforced.cwdDigest) || !DIGEST.test(input.enforced.loopbackMcpDigest) ||
      input.enforced.transport !== GOVERNED_CODEX_EXEC_TRANSPORT || input.enforced.codexHome !== 'omitted' ||
      input.enforced.environmentPolicy !== 'inherit-with-CODEX_HOME-omitted-v1' || input.enforced.source !== 'probe-host-codex-exec-argv/v1' ||
      input.enforced.ignoreUserConfig !== true || input.enforced.ignoreRules !== true || input.enforced.ephemeral !== true || input.enforced.noShell !== true ||
      !isAbsolute(input.enforced.cliPath) || !VERSION_TEXT.test(input.enforced.cliVersion) ||
      input.dispatch.source !== 'probe-host-exec' || input.dispatch.tool !== 'codex-exec' || !exactKeys(input.dispatch, ['promptBytes', 'promptDigest', 'source', 'tool']) || !DIGEST.test(input.dispatch.promptDigest) ||
      !Number.isSafeInteger(input.dispatch.promptBytes) || input.dispatch.promptBytes < 1 || input.dispatch.promptBytes > MAX_TEXT_BYTES || !exactKeys(input.evidence, ['agentMessageCount', 'completedItemCount', 'eventCount', 'probeMcpCallCount']) ||
      input.evidence.eventCount !== input.observed.eventCount || input.evidence.completedItemCount !== input.observed.completedItemCount ||
      input.evidence.agentMessageCount !== input.observed.agentMessageCount || input.evidence.probeMcpCallCount !== input.observed.probeMcpCallCount) throw new TypeError('Invalid governed Codex exec attestation');
  if (executableDigest(safeAbsoluteFile(input.enforced.cliPath, 'Codex executable')) !== input.enforced.cliSha256) throw new TypeError('Codex executable SHA-256 mismatch');
  if (input.enforced.cwdDigest !== input.requested.cwdDigest) throw new TypeError('Invalid governed Codex exec cwd binding');
  validateObserved(input.observed, input.profileId);
  validateUsageProjection(input.usage);
  if (input.executionContext !== undefined && (!exactKeys(input.executionContext, ['invocationDigest', 'source']) || input.executionContext.source !== 'caller' || !DIGEST.test(input.executionContext.invocationDigest))) throw new TypeError('Invalid governed Codex exec execution context');
  return freeze(input);
}

export const buildGovernedCodexExecAttestation = formatGovernedCodexExecAttestation;

function effectiveExecDispatch(prompt, instructionsDigest = null, instructionsBytes = 0) {
  const userDispatch = governedCodexDispatch(validatePrompt(prompt));
  if (instructionsDigest !== null) requireDigest(instructionsDigest, 'model instructions digest');
  if (!Number.isSafeInteger(instructionsBytes) || instructionsBytes < 0 || instructionsBytes > MAX_TEXT_BYTES) {
    throw new TypeError('Invalid model instructions byte count');
  }
  // The public byte count is the documented total of private instruction-file
  // bytes and exact user-prompt bytes; instruction text never crosses this boundary.
  const promptBytes = instructionsBytes + userDispatch.promptBytes;
  if (promptBytes < 1 || promptBytes > MAX_EFFECTIVE_INPUT_BYTES) throw new TypeError('Governed exec input exceeds byte bound');
  return Object.freeze({
    source: 'probe-host-exec', tool: 'codex-exec',
    promptDigest: sha256({ version: 'probe.governed-codex-exec-input/v1',
      instructions: { digest: instructionsDigest, bytes: instructionsBytes },
      user: { digest: userDispatch.promptDigest, bytes: userDispatch.promptBytes } }),
    promptBytes,
  });
}

export function previewGovernedCodexExecDispatch(prompt, systemPrompt = '') {
  if (typeof systemPrompt !== 'string' || systemPrompt.includes('\0') || Buffer.byteLength(systemPrompt, 'utf8') > MAX_TEXT_BYTES) {
    throw new TypeError('Invalid model instructions');
  }
  return effectiveExecDispatch(prompt, systemPrompt.length === 0 ? null : sha256(systemPrompt), Buffer.byteLength(systemPrompt, 'utf8'));
}

function validateThreadStarted(event, state) {
  if (!exactKeys(event, ['thread_id', 'type']) || event.type !== 'thread.started' ||
      typeof event.thread_id !== 'string' || !SAFE_ID.test(event.thread_id) || state.threadId !== null) {
    throw fail('EVENT_ORDER');
  }
  state.threadId = event.thread_id;
  state.phase = 'thread';
  state.streamRecords.push(Object.freeze({ event: 'thread.started', threadDigest: digestText(event.thread_id) }));
}

function validateTurnStarted(event, state) {
  if (!exactKeys(event, ['type']) || event.type !== 'turn.started' || state.phase !== 'thread') {
    throw fail('EVENT_ORDER');
  }
  state.turnStarted = true;
  state.phase = 'turn';
  state.streamRecords.push(Object.freeze({ event: 'turn.started' }));
}

function itemCategoryAllowed(type, profile) {
  if (PUBLIC_ITEM_TYPES.has(type)) return true;
  if (type === 'command_execution') return profile.version !== 'probe.governed-codex-profile/v1';
  if (type === 'file_change') return profile.version === 'probe.governed-codex-profile/v3';
  return false;
}

function mcpNameAllowed(name, profile) {
  const tools = profile.probeTools ?? profile.probeMcpTools ?? [];
  return typeof name === 'string' && tools.some(tool => name === `mcp__probe__${tool}`);
}

function validateItem(event, state, profile) {
  if (!exactKeys(event, ['item', 'type']) || !['item.started', 'item.completed'].includes(event.type) ||
      !ownObject(event.item) || typeof event.item.type !== 'string' || !itemCategoryAllowed(event.item.type, profile) ||
      state.phase !== 'turn') throw fail('EVENT_CATEGORY');
  const item = event.item;
  const allowedKeys = item.type === 'agent_message' ? ['id', 'phase', 'text', 'type']
    : item.type === 'reasoning' ? ['id', 'summary', 'text', 'type']
      : item.type === 'mcp_tool_call' ? ['arguments', 'id', 'name', 'result', 'server', 'status', 'type']
        : item.type === 'command_execution' ? ['aggregated_output', 'command', 'exit_code', 'id', 'status', 'type']
          : ['changes', 'id', 'status', 'type'];
  if (Object.keys(item).some(key => !allowedKeys.includes(key))) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_keys'));
  if (item.id !== undefined && (typeof item.id !== 'string' || !SAFE_ID.test(item.id))) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_id'));
  if ((item.type === 'agent_message' || item.type === 'reasoning') && item.text !== undefined &&
      (typeof item.text !== 'string' || Buffer.byteLength(item.text, 'utf8') > MAX_TEXT_BYTES)) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_text'));
  if (item.phase !== undefined && (typeof item.phase !== 'string' || !/^[A-Za-z0-9._:-]{1,64}$/.test(item.phase))) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_phase'));
  if (item.summary !== undefined && (!Array.isArray(item.summary) || item.summary.length > 32)) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_summary'));
  if (item.server !== undefined && (typeof item.server !== 'string' || !/^[A-Za-z0-9._:-]{1,128}$/.test(item.server))) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_server'));
  if (item.command !== undefined && (typeof item.command !== 'string' || Buffer.byteLength(item.command, 'utf8') > MAX_TEXT_BYTES)) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_command'));
  if (item.aggregated_output !== undefined && (typeof item.aggregated_output !== 'string' || Buffer.byteLength(item.aggregated_output, 'utf8') > MAX_TEXT_BYTES)) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_aggregated_output'));
  if (item.exit_code !== undefined && item.exit_code !== null && (!Number.isSafeInteger(item.exit_code) || item.exit_code < -255 || item.exit_code > 255)) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_exit_code'));
  if (item.changes !== undefined && (!Array.isArray(item.changes) || item.changes.length > 128)) throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_changes'));
  const isTool = item.type === 'mcp_tool_call' || NATIVE_ITEM_TYPES.has(item.type);
  if (isTool && (typeof item.id !== 'string' || !SAFE_ID.test(item.id))) throw fail('ITEM', undefined, rejectedItemEvent(event, 'tool_id'));
  let previous;
  if (item.id !== undefined) {
    previous = state.itemStates.get(item.id);
    if (event.type === 'item.started' && previous !== undefined) throw fail('DUPLICATE');
    if (event.type === 'item.completed' && (previous?.status === 'completed' || (previous && previous.type !== item.type))) throw fail('DUPLICATE');
    if (event.type === 'item.completed' && isTool && previous === undefined) throw fail('ITEM_ORDER');
    state.itemStates.set(item.id, { status: event.type === 'item.completed' ? 'completed' : 'started', type: item.type });
  }
  if (isTool) {
    if (item.type === 'mcp_tool_call') {
      if (!mcpNameAllowed(item.name, profile)) throw fail('TOOL_POLICY');
    } else if (item.name !== undefined && item.name !== null) throw fail('TOOL_POLICY');
    if (item.status !== undefined && !['in_progress', 'completed'].includes(item.status)) {
      throw fail('ITEM', undefined, rejectedItemEvent(event, 'item_status'));
    }
  }
  if (event.type === 'item.completed') {
    if (item.status !== undefined && item.status !== 'completed') throw fail('ITEM_STATUS');
    state.completedItemCount++;
    if (item.type === 'agent_message') {
      if (typeof item.text !== 'string') throw fail('ANSWER_CARDINALITY');
      state.answer = item.text;
      state.agentMessageCount++;
    }
    if (isTool) state.completedTools.push({
      category: item.type, name: item.type === 'mcp_tool_call' ? item.name : null,
    });
  }
  state.itemEvents++;
  state.streamRecords.push(Object.freeze({
    event: event.type, category: item.type,
    ...(item.id === undefined ? {} : { id: item.id }),
    ...(item.name === undefined ? {} : { name: item.name }),
    ...(item.status === undefined ? {} : { status: item.status }),
    ...(typeof item.text === 'string' ? { textDigest: digestText(item.text), textBytes: Buffer.byteLength(item.text, 'utf8') } : {}),
  }));
}

function validateUsage(value) {
  if (!ownObject(value) || Object.keys(value).some(key => !USAGE_KEYS.has(key)) ||
      !Object.prototype.hasOwnProperty.call(value, 'input_tokens') ||
      !Object.prototype.hasOwnProperty.call(value, 'output_tokens')) throw fail('USAGE');
  const usage = {};
  for (const key of Object.keys(value).sort()) {
    const count = value[key];
    if (!Number.isSafeInteger(count) || count < 0 || count > Number.MAX_SAFE_INTEGER) throw fail('USAGE');
    usage[key] = count;
  }
  return Object.freeze(usage);
}

function validateTurnCompleted(event, state) {
  if (!exactKeys(event, ['type', 'usage']) || event.type !== 'turn.completed' || state.phase !== 'turn' ||
      state.answer === null || state.turnCompleted) throw fail('EVENT_ORDER');
  for (const item of state.itemStates.values()) {
    if (item.status === 'started' && (item.type === 'mcp_tool_call' || NATIVE_ITEM_TYPES.has(item.type))) throw fail('INCOMPLETE_ITEM');
  }
  state.usage = validateUsage(event.usage);
  state.streamRecords.push(Object.freeze({ event: 'turn.completed', usage: state.usage }));
  state.turnCompleted = true;
  state.phase = 'completed';
}

function consumeEvent(event, state, profile) {
  if (!ownObject(event) || typeof event.type !== 'string') throw fail('EVENT');
  if (state.turnCompleted) throw fail('EVENT_ORDER');
  state.events++;
  if (state.events > MAX_EVENTS) throw fail('EVENT_LIMIT');
  if (state.phase === 'initial') return validateThreadStarted(event, state);
  if (state.phase === 'thread') return validateTurnStarted(event, state);
  if (event.type === 'item.started' || event.type === 'item.completed') return validateItem(event, state, profile);
  if (event.type === 'turn.completed') return validateTurnCompleted(event, state);
  throw fail('EVENT_CATEGORY');
}

function parserState() {
  return {
    phase: 'initial', threadId: null, turnStarted: false, turnCompleted: false,
    answer: null, usage: null, events: 0, itemEvents: 0, completedItemCount: 0,
    agentMessageCount: 0, completedTools: [], streamRecords: [], itemStates: new Map(),
  };
}

function aggregateTools(state) {
  const counts = new Map();
  for (const item of state.completedTools) {
    const key = `${item.category}\u0000${item.name ?? ''}`;
    counts.set(key, { ...item, status: 'completed', count: (counts.get(key)?.count ?? 0) + 1 });
  }
  return [...counts.values()].sort((a, b) => `${a.category}:${a.name ?? ''}`.localeCompare(`${b.category}:${b.name ?? ''}`));
}

function makeInternalResult(state, processReceipt, launch, invocationDigest, mcpEvidence) {
  if (state.threadId === null || !state.turnStarted || !state.turnCompleted || state.answer === null || !state.usage) {
    throw fail('INCOMPLETE');
  }
  if (processReceipt.classification !== 'exited' || processReceipt.exitCode !== 0 || processReceipt.signal !== null ||
      !processReceipt.barriers.stdoutEOF || !processReceipt.barriers.stderrEOF || !processReceipt.barriers.close) {
    throw fail(processReceipt.classification === 'execution_timeout' ? 'TIMEOUT' : 'EXIT', processReceipt.stderr);
  }
  const usedToolItems = aggregateTools(state);
  const probeMcpCallCount = mcpEvidence.closed;
  if (probeMcpCallCount !== usedToolItems.filter(item => item.category === 'mcp_tool_call').reduce((sum, item) => sum + item.count, 0)) {
    throw fail('MCP_EVIDENCE');
  }
  const observed = {
    source: 'codex-exec-jsonl/v1', threadDigest: sha256({ version: 'probe.governed-codex-thread/v1', threadId: state.threadId }),
    streamDigest: sha256({ version: 'probe.governed-codex-stream/v1', events: state.streamRecords }), terminal: 'turn.completed',
    eventCount: state.events, completedItemCount: state.completedItemCount, agentMessageCount: state.agentMessageCount,
    usedToolItems, probeMcpCallCount, finalDigest: digestText(state.answer), finalBytes: Buffer.byteLength(state.answer, 'utf8'),
    processExitCode: 0, processSignal: null,
  };
  const attestation = formatGovernedCodexExecAttestation({
    profile: launch.profile, cliPath: launch.command, cliSha256: launch.cliSha256, cliVersion: launch.cliVersion,
    configDigest: launch.configDigest, launchDigest: launch.launchDigest, cwdDigest: launch.requested.cwdDigest, loopbackMcpDigest: launch.loopbackMcpDigest,
    promptDigest: launch.promptDigest, promptBytes: launch.promptBytes, observed,
    usage: { status: 'observed', inputTokens: state.usage.input_tokens, cachedInputTokens: state.usage.cached_input_tokens ?? 0, outputTokens: state.usage.output_tokens },
    invocationDigest,
  });
  return Object.freeze({
    version: GOVERNED_CODEX_EXEC_PROTOCOL,
    answer: state.answer,
    threadId: state.threadId,
    usage: state.usage,
    attestation,
    evidence: Object.freeze({ eventCount: state.events, completedItemCount: state.completedItemCount,
      agentMessageCount: state.agentMessageCount, usedToolItems: Object.freeze(usedToolItems.map(item => Object.freeze(item))), probeMcpCallCount }),
    process: Object.freeze({ classification: processReceipt.classification, exitCode: processReceipt.exitCode, signal: processReceipt.signal }),
  });
}

function processFailure(receipt) {
  if (receipt.classification === 'execution_timeout') return fail('TIMEOUT', receipt.stderr);
  if (receipt.classification === 'aborted') return fail('CANCELLED', receipt.stderr);
  if (receipt.classification === 'output_overflow') return fail('OUTPUT_OVERFLOW', receipt.stderr);
  if (receipt.classification === 'spawn_error') return fail('SPAWN', receipt.stderr);
  if (receipt.classification === 'cleanup_timeout') return fail('CLEANUP', receipt.stderr);
  if (receipt.signal) return fail('EXIT', receipt.stderr);
  return null;
}

/**
 * Create a governed single-query exec engine. `run()` may be called once;
 * `close()` is idempotent and must be used by callers that do not run it.
 */
export async function createGovernedCodexExecEngine(options = {}) {
  if (!ownObject(options)) throw new TypeError('Invalid governed Codex exec options');
  if (!options.agent || typeof options.agent !== 'object') throw new TypeError('Invalid governed Codex exec agent');
  const profile = validateGovernedCodexProfile(options.profile);
  const prompt = options.prompt === undefined ? null : validatePrompt(options.prompt);
  const systemPrompt = options.systemPrompt === undefined ? '' : validatePrompt(options.systemPrompt);
  const codexPath = safeAbsoluteFile(options.codexPath, 'Codex executable');
  const codexSha256 = normalizeCliDigest(options.codexSha256);
  const timeoutMs = validateTimeout(options.timeoutMs);
  const executionTimeoutMs = validateTimeout(options.executionTimeoutMs ?? timeoutMs);
  validateSignal(options.signal);
  const actualSha = executableDigest(codexPath);
  if (actualSha !== codexSha256) throw new TypeError('Codex executable SHA-256 mismatch');
  const cliVersion = await verifyCodexVersion(codexPath, profile.cwd, options.signal, timeoutMs);

  const sessionId = randomBytes(8).toString('hex');
  const mcpServer = new BuiltInMCPServer(options.agent, {
    port: 0, host: '127.0.0.1', debug: false,
    // Exec transport receipts count Probe MCP calls for every governed profile;
    // the server option only enables bounded call accounting and does not widen
    // the set of tools exposed to Codex.
    governedProfileVersion: 'probe.governed-codex-profile/v2',
  });
  let processHandle = null;
  let runPromise = null;
  let closePromise = null;
  let processReceipt = null;
  let launch = null;
  let primaryError = null;
  let state = parserState();
  let instructionDirectory = null;
  let instructionPath = null;
  let instructionDigest = null;

  try {
    if (systemPrompt) {
      instructionDirectory = mkdtempSync(join(tmpdir(), 'probe-governed-exec-instructions-'));
      instructionPath = join(instructionDirectory, 'model-instructions.txt');
      writeFileSync(instructionPath, systemPrompt, { mode: 0o600, flag: 'wx' });
      instructionDigest = sha256(systemPrompt);
    }
    await mcpServer.start();
    const binding = mcpServer.getConfig();
    const run = (runPrompt = prompt, runOptions = {}) => {
      if (runPromise) return Promise.reject(fail('ONE_QUERY'));
      const activePrompt = validatePrompt(runPrompt);
      const activeSignal = runOptions.abortSignal ?? options.signal;
      validateSignal(activeSignal);
      launch = buildGovernedCodexExecLaunch({
        codexPath, codexSha256, cliVersion, profile, prompt: activePrompt,
        mcp: { name: `probe_${sessionId}`, url: binding.url },
        ...(instructionPath ? { modelInstructionsPath: instructionPath, modelInstructionsDigest: instructionDigest } : {}),
      });
      runPromise = (async () => {
        let child;
        try {
          child = spawn(launch.command, launch.args, {
            cwd: launch.cwd,
            env: cloneEnvironment(),
            stdio: ['ignore', 'pipe', 'pipe'],
            shell: false,
            detached: true,
            windowsHide: true,
          });
        } catch {
          throw fail('SPAWN');
        }
        processHandle = governSpawnedProcess(child, {
          captureStdout: false,
          stdoutByteCap: STDOUT_BYTE_CAP,
          stderrByteCap: STDERR_BYTE_CAP,
          signalScope: 'process-group',
          executionTimeoutMs,
            signal: activeSignal,
        });
        const decoder = new StringDecoder('utf8');
        let pending = '';
        let stdoutBytes = 0;
        const consumeLine = line => {
          if (line.endsWith('\r')) line = line.slice(0, -1);
          if (line.length === 0) throw fail('JSONL');
          if (Buffer.byteLength(line, 'utf8') > JSONL_LINE_BYTE_CAP) throw fail('OUTPUT_OVERFLOW');
          let event;
          try { event = JSON.parse(line); } catch { throw fail('JSONL'); }
            consumeEvent(event, state, profile);
        };
        child.stdout.on('data', chunk => {
          if (primaryError) return;
          const bytes = Buffer.isBuffer(chunk) ? chunk.length : Buffer.byteLength(String(chunk), 'utf8');
          stdoutBytes += bytes;
          if (stdoutBytes > STDOUT_BYTE_CAP) {
            primaryError = fail('OUTPUT_OVERFLOW');
            void processHandle.terminate('stdout_overflow');
            return;
          }
          try {
            pending += decoder.write(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
            let newline;
            while ((newline = pending.indexOf('\n')) >= 0) {
              const line = pending.slice(0, newline);
              pending = pending.slice(newline + 1);
              consumeLine(line);
            }
            if (Buffer.byteLength(pending, 'utf8') > JSONL_LINE_BYTE_CAP) throw fail('OUTPUT_OVERFLOW');
          } catch (error) {
            primaryError = error instanceof Error && error.code?.startsWith('GOVERNED_CODEX_EXEC_') ? error : fail('JSONL');
            void processHandle.terminate(primaryError.code);
          }
        });
        child.stdout.once('end', () => {
          if (primaryError) return;
          try {
            pending += decoder.end();
            if (pending.length > 0) consumeLine(pending);
          } catch (error) {
            primaryError = error instanceof Error && error.code?.startsWith('GOVERNED_CODEX_EXEC_') ? error : fail('JSONL');
            void processHandle.terminate(primaryError.code);
          }
        });
        processReceipt = await processHandle.result;
        if (primaryError) {
          if (processReceipt.stderr) {
            const stderr = summarizeStderr(processReceipt.stderr);
            if (stderr && !primaryError.diagnostic) Object.defineProperty(primaryError, 'diagnostic', { value: stderr, enumerable: true });
          }
          throw primaryError;
        }
        const receiptFailure = processFailure(processReceipt);
        if (receiptFailure) throw receiptFailure;
        const evidence = mcpServer.getGovernedCallEvidence();
        return makeInternalResult(state, processReceipt, launch, runOptions.invocationDigest ?? options.invocationDigest, evidence);
      })();
      return runPromise;
    };

    const close = () => {
      if (closePromise) return closePromise;
      closePromise = (async () => {
        if (processHandle && !processReceipt) {
          try { processReceipt = await processHandle.terminate('closed'); } catch { if (!primaryError) primaryError = fail('CLEANUP'); }
        }
        try { await mcpServer.stop(); } catch { if (!primaryError) primaryError = fail('CLEANUP'); }
        if (instructionDirectory) { try { rmSync(instructionDirectory, { recursive: true, force: true }); } catch { if (!primaryError) primaryError = fail('CLEANUP'); } }
        if (primaryError && !runPromise) throw primaryError;
      })();
      return closePromise;
    };

    const query = async function* query(queryPrompt, queryOptions = {}) {
      const requestedPrompt = validatePrompt(queryPrompt);
      const result = await run(requestedPrompt, queryOptions);
      yield { type: 'text', content: result.answer };
      yield { type: 'metadata', data: { attestation: result.attestation, candidateBoundary: {
        selectedOrigin: result.attestation.observed.finalBytes > 0 ? 'raw_final' : 'none',
        selectedChunkCount: result.attestation.observed.finalBytes > 0 ? 1 : 0,
        selectedBytes: result.attestation.observed.finalBytes,
        resultTextItemCount: 0, resultTextBytes: 0,
        rawFinalMessageCount: result.attestation.observed.agentMessageCount,
        rawFinalPartCount: result.attestation.observed.agentMessageCount,
        rawFinalBytes: result.attestation.observed.finalBytes,
      } } };
    };
    return Object.freeze({ run, query, close, get launch() {
      return launch && Object.freeze({ ...launch, args: Object.freeze([...launch.args]) });
    } });
  } catch (error) {
    try { await mcpServer.stop(); } catch { /* preserve the setup failure */ }
    if (instructionDirectory) { try { rmSync(instructionDirectory, { recursive: true, force: true }); } catch { /* preserve setup failure */ } }
    if (error instanceof TypeError || error?.code?.startsWith('GOVERNED_CODEX_EXEC_')) throw error;
    throw fail('SETUP');
  }
}

export async function runGovernedCodexExec(options = {}) {
  const engine = await createGovernedCodexExecEngine(options);
  try {
    return await engine.run();
  } finally {
    await engine.close();
  }
}
