/**
 * OpenAI Codex Engine using MCP server approach with event streaming
 * Runs 'codex mcp-server' and handles codex/event notifications
 */

import { spawn } from 'child_process';
import { createHash, randomBytes } from 'crypto';
import { createInterface } from 'readline';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { governSpawnedProcess } from '../processSupervisor.js';
import { Session } from '../shared/Session.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs, validateGovernedCodexProfile } from './governed-codex-profile.js';
import { governedAnswerFailure, normalizeGovernedAnswerFailure } from './governed-answer-failure.js';

const GOVERNED_NATIVE_EVENT_LIMIT = 256;
const GOVERNED_SAFE_ID = /^[A-Za-z0-9._:-]{1,128}$/;
const GOVERNED_SAFE_KIND = /^[A-Za-z0-9._:-]{1,64}$/;
const GOVERNED_CODEX_NATIVE_CALLS = new Map([['exec', 'exec']]);
const GOVERNED_PROBE_MCP_CALLS = new Map([
  ['mcp__probe__search', 'search'], ['mcp__probe__extract', 'extract'], ['mcp__probe__listFiles', 'listFiles'],
]);

function governedRawItemInvalid() { throw governedAnswerFailure('native_event_grammar', 'raw_item_predicate'); }
function governedLiveEnvelopeInvalid(subreason, correlationOperand = null) {
  throw governedAnswerFailure('native_event_grammar', 'live_envelope_session', subreason, correlationOperand);
}
function governedExactObject(value, keys, invalid = governedRawItemInvalid) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) invalid();
  const proto = Object.getPrototypeOf(value);
  if (proto !== Object.prototype && proto !== null) invalid();
  const actual = Reflect.ownKeys(value).filter((key) => Object.prototype.propertyIsEnumerable.call(value, key));
  if (actual.some((key) => typeof key !== 'string') || actual.length !== keys.length || keys.some((key) => !actual.includes(key))) invalid();
  for (const key of keys) if (!Object.prototype.hasOwnProperty.call(Object.getOwnPropertyDescriptor(value, key), 'value')) invalid();
  return value;
}
function governedSafeId(value) { if (typeof value !== 'string' || !GOVERNED_SAFE_ID.test(value)) governedRawItemInvalid(); }
function governedPassthrough(value, message = false) {
  const keys = Object.keys(value ?? {}).sort().join(',');
  const legacy = keys === 'turn_id';
  const current = keys === (message ? 'content_item_kinds,create_time,turn_id' : 'create_time,turn_id');
  if (!legacy && !current) governedRawItemInvalid();
  governedSafeId(value.turn_id);
  if (current) {
    if (typeof value.create_time !== 'number' || !Number.isFinite(value.create_time) ||
      value.create_time < 0 || value.create_time > Number.MAX_SAFE_INTEGER) governedRawItemInvalid();
    if (message) {
      if (!Array.isArray(value.content_item_kinds) || value.content_item_kinds.length > 16) governedRawItemInvalid();
      for (const kind of value.content_item_kinds) if (typeof kind !== 'string' || !GOVERNED_SAFE_KIND.test(kind)) governedRawItemInvalid();
    }
  }
}
function validateGovernedRawMessage(item) {
  const assistant = item.role === 'assistant';
  const keys = assistant
    ? ['type', 'id', 'role', 'content', 'phase', 'internal_chat_message_metadata_passthrough']
    : Object.prototype.hasOwnProperty.call(item ?? {}, 'id')
      ? ['type', 'id', 'role', 'content', 'internal_chat_message_metadata_passthrough']
      : ['type', 'role', 'content', 'internal_chat_message_metadata_passthrough'];
  governedExactObject(item, keys);
  if (item.type !== 'message' || !['developer', 'user', 'assistant'].includes(item.role)) governedRawItemInvalid();
  if (Object.prototype.hasOwnProperty.call(item, 'id')) governedSafeId(item.id);
  if (assistant && !['commentary', 'final_answer'].includes(item.phase)) governedRawItemInvalid();
  if (!Array.isArray(item.content) || item.content.length < 1 || item.content.length > 64) governedRawItemInvalid();
  for (const part of item.content) {
    governedExactObject(part, ['type', 'text']);
    const allowed = assistant ? part.type === 'output_text' : part.type === 'input_text';
    if (!allowed || typeof part.text !== 'string' || Buffer.byteLength(part.text, 'utf8') > 131072) governedRawItemInvalid();
  }
  governedPassthrough(item.internal_chat_message_metadata_passthrough, true);
}
function validateGovernedRawReasoning(item) {
  governedExactObject(item, ['type', 'id', 'summary', 'encrypted_content', 'internal_chat_message_metadata_passthrough']);
  if (item.type !== 'reasoning') governedRawItemInvalid(); governedSafeId(item.id);
  if (!Array.isArray(item.summary) || item.summary.length !== 0 || typeof item.encrypted_content !== 'string' || Buffer.byteLength(item.encrypted_content, 'utf8') > 1048576) governedRawItemInvalid();
  governedPassthrough(item.internal_chat_message_metadata_passthrough);
}
function createGovernedNativeCollector(profile) {
  let sessionEvent = null, requestId = null, threadId = null, nativeCallCount = 0, probeMcpCallCount = 0;
  let relevantEventCount = 0, totalCallCount = 0, rawResponseItemCount = 0, assistantMessageCount = 0, finalAnswerCount = 0;
  const rawIds = new Set(), callOrigins = new Map(), outputIds = new Set();
  function observe(event) {
    const type = event?.params?.msg?.type;
    if (type === 'session_configured') {
      if (sessionEvent) governedLiveEnvelopeInvalid('session_sequence');
      sessionEvent = event;
      requestId = event.params?._meta?.requestId;
      threadId = event.params?._meta?.threadId;
      return;
    }
    if (profile.version !== 'probe.governed-codex-profile/v2' || type !== 'raw_response_item') return;
    if (!sessionEvent) governedLiveEnvelopeInvalid('session_sequence');
    governedExactObject(event, ['jsonrpc', 'method', 'params'], () => governedLiveEnvelopeInvalid('envelope_shape'));
    if (event.jsonrpc !== '2.0' || event.method !== 'codex/event') governedLiveEnvelopeInvalid('envelope_shape');
    const params = governedExactObject(event.params, ['_meta', 'msg', 'id'], () => governedLiveEnvelopeInvalid('envelope_shape'));
    const meta = governedExactObject(params._meta, ['requestId', 'threadId'], () => governedLiveEnvelopeInvalid('envelope_shape'));
    if (meta.requestId !== requestId) governedLiveEnvelopeInvalid('correlation');
    if (meta.threadId !== threadId) governedLiveEnvelopeInvalid('correlation', 'thread_id');
    if (params.id !== String(requestId)) governedLiveEnvelopeInvalid('correlation', 'response_id');
    const msg = governedExactObject(params.msg, ['type', 'item'], () => governedLiveEnvelopeInvalid('envelope_shape'));
    if (msg.type !== 'raw_response_item') governedLiveEnvelopeInvalid('envelope_shape');
    const item = msg.item;
    if (++rawResponseItemCount > GOVERNED_NATIVE_EVENT_LIMIT) governedRawItemInvalid();
    if (item?.type === 'message') {
      validateGovernedRawMessage(item);
      if (Object.prototype.hasOwnProperty.call(item, 'id')) {
        if (rawIds.has(item.id)) governedRawItemInvalid(); rawIds.add(item.id);
      }
      if (item.role === 'assistant') { assistantMessageCount++; if (item.phase === 'final_answer') finalAnswerCount++; }
      return;
    }
    if (item?.type === 'reasoning') {
      validateGovernedRawReasoning(item);
      if (rawIds.has(item.id)) governedRawItemInvalid(); rawIds.add(item.id);
      return;
    }
    if (item?.type === 'custom_tool_call') {
      governedExactObject(item, ['type', 'id', 'status', 'call_id', 'name', 'input', 'internal_chat_message_metadata_passthrough']);
      governedSafeId(item.id); governedSafeId(item.call_id);
      if (rawIds.has(item.id) || callOrigins.has(item.call_id)) governedRawItemInvalid();
      const nativeName = GOVERNED_CODEX_NATIVE_CALLS.get(item.name);
      const probeMcpName = GOVERNED_PROBE_MCP_CALLS.get(item.name);
      if ((nativeName !== undefined) === (probeMcpName !== undefined)) governedRawItemInvalid();
      if (nativeName !== undefined && !profile.codexNativeTools.includes(nativeName)) governedRawItemInvalid();
      if (probeMcpName !== undefined && !profile.probeMcpTools.includes(probeMcpName)) governedRawItemInvalid();
      if (item.status !== 'completed' || typeof item.input !== 'string' || Buffer.byteLength(item.input, 'utf8') > 131072) governedRawItemInvalid();
      governedPassthrough(item.internal_chat_message_metadata_passthrough);
      if (++relevantEventCount > GOVERNED_NATIVE_EVENT_LIMIT || ++totalCallCount > GOVERNED_NATIVE_EVENT_LIMIT) governedRawItemInvalid();
      const origin = nativeName !== undefined ? 'codex-native' : 'probe-mcp';
      rawIds.add(item.id); callOrigins.set(item.call_id, origin);
      if (origin === 'codex-native') nativeCallCount++; else probeMcpCallCount++;
      return;
    }
    if (item?.type === 'custom_tool_call_output') {
      const hasId = Object.prototype.hasOwnProperty.call(item, 'id');
      governedExactObject(item, hasId
        ? ['type', 'id', 'call_id', 'output', 'internal_chat_message_metadata_passthrough']
        : ['type', 'call_id', 'output', 'internal_chat_message_metadata_passthrough']);
      if (hasId) { governedSafeId(item.id); if (rawIds.has(item.id)) governedRawItemInvalid(); }
      governedSafeId(item.call_id);
      if (!callOrigins.has(item.call_id) || outputIds.has(item.call_id) || !Array.isArray(item.output) || item.output.length > 64) governedRawItemInvalid();
      for (const part of item.output) {
        governedExactObject(part, ['type', 'text']);
        if (part.type !== 'input_text' || typeof part.text !== 'string' || Buffer.byteLength(part.text, 'utf8') > 1048576) governedRawItemInvalid();
      }
      governedPassthrough(item.internal_chat_message_metadata_passthrough);
      if (++relevantEventCount > GOVERNED_NATIVE_EVENT_LIMIT) governedRawItemInvalid();
      if (hasId) rawIds.add(item.id);
      outputIds.add(item.call_id);
      return;
    }
    governedRawItemInvalid();
  }
  function evidence() {
    if (!sessionEvent) governedLiveEnvelopeInvalid('session_sequence');
    if (assistantMessageCount > 0 && finalAnswerCount !== 1) governedRawItemInvalid();
    const tools = nativeCallCount === 0 ? [] : [{ name: 'exec', status: 'completed', count: nativeCallCount }];
    return { sessionEvent, capabilities: { nativeTools: { total: nativeCallCount, tools }, probeMcpCallCount } };
  }
  return { observe, evidence };
}

function externalReceipt(attestation, capabilityCounts) {
  if (attestation.version === 'probe.governed-codex-attestation/v3') return externalV3Receipt(attestation, {}, capabilityCounts);
  return {
    version: attestation.version, profileId: attestation.profileId,
    requested: { profileDigest: attestation.requested.profileDigest, cwdDigest: attestation.requested.cwdDigest, probeToolsDigest: attestation.requested.probeToolsDigest, model: attestation.requested.model, reasoningEffort: attestation.requested.reasoningEffort, sandbox: attestation.requested.sandbox, approvalPolicy: attestation.requested.approvalPolicy },
    observed: { source: attestation.observed.source, model: attestation.observed.model, modelProviderId: attestation.observed.modelProviderId, reasoningEffort: attestation.observed.reasoningEffort, approvalPolicy: attestation.observed.approvalPolicy, cwdDigest: attestation.observed.cwdDigest, permissionProfileDigest: attestation.observed.permissionProfileDigest, filesystem: attestation.observed.filesystem, network: attestation.observed.network },
    evidence: { eventCount: 1 }, usage: { status: 'unavailable' },
  };
}

function externalV3Receipt(attestation, extra, capabilityCounts) {
  return {
    version: attestation.version, profileId: attestation.profileId,
    requested: { profileDigest: attestation.requested.profileDigest, cwdDigest: attestation.requested.cwdDigest,
      probeMcpToolsDigest: attestation.requested.probeMcpToolsDigest, codexNativeToolsDigest: attestation.requested.codexNativeToolsDigest,
      probeMcpTools: [...attestation.requested.probeMcpTools], codexNativeTools: [...attestation.requested.codexNativeTools],
      model: attestation.requested.model, reasoningEffort: attestation.requested.reasoningEffort,
      sandbox: attestation.requested.sandbox, approvalPolicy: attestation.requested.approvalPolicy },
    observed: { source: attestation.observed.source, model: attestation.observed.model,
      modelProviderId: attestation.observed.modelProviderId, reasoningEffort: attestation.observed.reasoningEffort,
      approvalPolicy: attestation.observed.approvalPolicy, cwdDigest: attestation.observed.cwdDigest,
      permissionProfileDigest: attestation.observed.permissionProfileDigest, filesystem: attestation.observed.filesystem,
      network: attestation.observed.network, nativeTools: { total: attestation.observed.nativeTools.total,
        tools: attestation.observed.nativeTools.tools.map((item) => ({ ...item })) } },
    ...extra, evidence: { sessionEventCount: 1, nativeCallCount: capabilityCounts.nativeCallCount,
      probeMcpCallCount: capabilityCounts.probeMcpCallCount }, usage: { status: 'unavailable' },
  };
}

export function governedCodexDispatch(prompt) {
  const promptBytes = Buffer.byteLength(prompt, 'utf8');
  const byteLength = Buffer.alloc(8);
  byteLength.writeBigUInt64BE(BigInt(promptBytes));
  const promptDigest = `sha256:${createHash('sha256')
    .update('probe.governed-codex-dispatch/prompt/v1', 'utf8')
    .update(Buffer.from([0])).update(byteLength).update(prompt, 'utf8').digest('hex')}`;
  return Object.freeze({ source: 'probe-host-tools-call', tool: 'codex', promptDigest, promptBytes });
}

export function composeCodexInitialPrompt({ systemPrompt, customPrompt, prompt }) {
  const fullPrompt = combinePrompts(systemPrompt, customPrompt);
  return fullPrompt ? `${fullPrompt}\n\n${prompt}` : prompt;
}

export function previewGovernedCodexInitialDispatch(input) {
  return governedCodexDispatch(composeCodexInitialPrompt(input));
}

function externalBoundReceipt(internal, dispatch, invocationDigest, capabilityCounts) {
  if (internal.version === 'probe.governed-codex-attestation/v3') return externalV3Receipt(internal, {
    executionContext: { source: 'caller', invocationDigest },
    dispatch: { source: dispatch.source, tool: dispatch.tool, promptDigest: dispatch.promptDigest, promptBytes: dispatch.promptBytes },
  }, capabilityCounts);
  return {
    version: 'probe.governed-codex-attestation/v2', profileId: 'luna-xhigh-readonly-v1',
    requested: { profileDigest: internal.requested.profileDigest, cwdDigest: internal.requested.cwdDigest, probeToolsDigest: internal.requested.probeToolsDigest, model: internal.requested.model, reasoningEffort: internal.requested.reasoningEffort, sandbox: internal.requested.sandbox, approvalPolicy: internal.requested.approvalPolicy },
    observed: { source: internal.observed.source, model: internal.observed.model, modelProviderId: internal.observed.modelProviderId, reasoningEffort: internal.observed.reasoningEffort, approvalPolicy: internal.observed.approvalPolicy, cwdDigest: internal.observed.cwdDigest, permissionProfileDigest: internal.observed.permissionProfileDigest, filesystem: internal.observed.filesystem, network: internal.observed.network },
    executionContext: { source: 'caller', invocationDigest },
    dispatch: { source: dispatch.source, tool: dispatch.tool, promptDigest: dispatch.promptDigest, promptBytes: dispatch.promptBytes },
    evidence: { eventCount: 1 }, usage: { status: 'unavailable' },
  };
}

/**
 * Codex Engine using MCP Server with event streaming
 */
export async function createCodexEngine(options = {}) {
  const { agent, systemPrompt, customPrompt, debug, sessionId, allowedTools, model } = options;
  const governedProfile = options.governedCodexProfile === undefined ? null : validateGovernedCodexProfile(options.governedCodexProfile);

  const session = new Session(
    sessionId || randomBytes(8).toString('hex'),
    debug
  );

  // Start built-in MCP server for Probe tools
  let mcpServer = null;
  let mcpServerUrl = null;
  let mcpServerName = null;

  if (agent) {
    mcpServer = new BuiltInMCPServer(agent, {
      port: 0,
      host: '127.0.0.1',
      debug: debug
    });

    const { host, port } = await mcpServer.start();
    mcpServerUrl = `http://${host}:${port}/mcp`;
    mcpServerName = `probe_${session.id}`;

    if (debug) {
      console.log('[DEBUG] Built-in Probe MCP server started');
      console.log('[DEBUG] Probe MCP URL:', mcpServerUrl);
    }
  }

  // Start Codex MCP server
  if (debug) {
    console.log('[DEBUG] Starting Codex MCP server...');
  }

  const codexProcess = spawn('codex', ['mcp-server'], {
    stdio: ['pipe', 'pipe', 'pipe'],
    detached: true
  });
  const governedProcess = governSpawnedProcess(codexProcess, {
    captureStdout: false,
    signalScope: 'process-group',
    stdoutByteCap: 0
  });

  // Setup JSON-RPC communication
  let requestId = 0;
  const pendingRequests = new Map();
  const eventHandlers = new Map();
  const governedEvidenceHandlers = new Map();
  let governedQueryStarted = false, closePromise = null;

  // Read stdout line by line
  const stdoutReader = createInterface({
    input: codexProcess.stdout,
    crlfDelay: Infinity
  });

  stdoutReader.on('line', (line) => {
    try {
      const message = JSON.parse(line);

      if (debug) {
        if (message.method === 'codex/event') {
          console.log(`[DEBUG] Codex event: ${message.params?.msg?.type}`);
        }
      }

      // Handle responses to our requests
      if (message.id !== undefined && pendingRequests.has(message.id)) {
        const { resolve, reject, timer } = pendingRequests.get(message.id);
        pendingRequests.delete(message.id);
        clearTimeout(timer);

        if (message.error) {
          reject(new Error(message.error.message || JSON.stringify(message.error)));
        } else {
          resolve(message.result);
        }
      }

      // Handle notifications (codex/event)
      if (message.method === 'codex/event' && message.params) {
        const requestId = message.params._meta?.requestId;
        if (requestId !== undefined && governedEvidenceHandlers.has(requestId)) {
          governedEvidenceHandlers.get(requestId)(message);
        }
        if (requestId !== undefined && eventHandlers.has(requestId)) {
          eventHandlers.get(requestId)(message.params);
        }
      }
    } catch (e) {
      if (debug) {
        console.error('[DEBUG] Failed to parse message:', line);
      }
    }
  });

  codexProcess.once('error', (error) => rejectPending(error)); codexProcess.once('close', () => rejectPending(new Error('Codex process closed')));

  // Handle stderr
  if (debug) {
    codexProcess.stderr.on('data', (data) => {
      console.error('[CODEX STDERR]', data.toString());
    });
  }

  // Send JSON-RPC request
  function sendRequest(method, params = {}) {
    return new Promise((resolve, reject) => {
      if (closePromise) return reject(new Error('Codex engine is closed'));
      const id = ++requestId;
      const request = {
        jsonrpc: '2.0',
        id,
        method,
        params
      };

      // Timeout after 10 minutes
      const timer = setTimeout(() => {
        if (pendingRequests.has(id)) {
          pendingRequests.delete(id);
          reject(new Error(`Request ${method} timed out after 10 minutes`));
        }
      }, 600000);
      pendingRequests.set(id, { resolve, reject, timer });

      codexProcess.stdin.write(JSON.stringify(request) + '\n');
    });
  }

  function rejectPending(error) { for (const { reject, timer } of pendingRequests.values()) { clearTimeout(timer); reject(error); } pendingRequests.clear(); }

  async function cleanup(reason = new Error('Codex engine closed')) {
    if (closePromise) return closePromise;
    closePromise = (async () => {
      rejectPending(reason); eventHandlers.clear(); governedEvidenceHandlers.clear();
      stdoutReader.close(); codexProcess.stdin.destroy();
      const receipt = await governedProcess.terminate('codex_engine_closed');
      const processCleanupFailed = receipt.classification === 'cleanup_timeout' ||
        !receipt.barriers.close || !receipt.barriers.stdoutEOF || !receipt.barriers.stderrEOF;
      if (mcpServer) await mcpServer.stop();
      if (processCleanupFailed) throw new Error('Codex process cleanup failed');
    })();
    return closePromise;
  }

  // Initialize MCP connection
  try {
    await sendRequest('initialize', {
      protocolVersion: '2024-11-05', capabilities: { tools: {} },
      clientInfo: { name: 'probe-codex-client', version: '1.0.0' }
    });
  } catch (error) { await cleanup(error); throw error; }

  if (debug) {
    console.log('[DEBUG] Connected to Codex MCP server');
    console.log('[DEBUG] Session:', session.id);
  }

  return {
    sessionId: session.id,
    session,

    /**
     * Query Codex via MCP protocol with event streaming
     */
    async *query(prompt, opts = {}) {
      // Build prompt
      let finalPrompt = prompt;
      if (!session.conversationId) {
        finalPrompt = composeCodexInitialPrompt({ systemPrompt, customPrompt, prompt });
      }

      const isFollowUp = session.conversationId !== null;
      const toolName = isFollowUp ? 'codex-reply' : 'codex';
      let abortHandler, queryError = null;

      try {
        const hasInvocationDigest = Object.prototype.hasOwnProperty.call(opts,
          'invocationDigest');
        const invocationDigest = hasInvocationDigest ? opts.invocationDigest : undefined;
        if (hasInvocationDigest && (typeof invocationDigest !== 'string' || !/^sha256:[0-9a-f]{64}$/.test(invocationDigest))) {
          throw new TypeError('answerGoverned invocationDigest must match sha256:<64 lowercase hexadecimal digits>');
        }

        // Build arguments
        let toolArgs = { prompt: finalPrompt };

      if (isFollowUp) {
        toolArgs.conversationId = session.conversationId;
        if (debug) {
          console.log(`[DEBUG] Follow-up with conversationId: ${session.conversationId}`);
        }
      } else {
        if (governedProfile) {
          if (governedQueryStarted) throw new Error('Governed Codex engine permits one initial query');
          governedQueryStarted = true;
          toolArgs = buildGovernedCodexInitialToolArgs({ profile: governedProfile, prompt: finalPrompt, mcp: { name: mcpServerName, url: mcpServerUrl } });
        } else if (model) {
          toolArgs.model = model;
        }
        if (!governedProfile && mcpServerUrl && mcpServerName) {
          toolArgs.config = {
            mcp_servers: {
              [mcpServerName]: { url: mcpServerUrl }
            }
          };
        }
        if (debug) {
          console.log(`[DEBUG] Initial query with tool: ${toolName}`);
        }
      }

        const reqId = requestId + 1;
        let fullResponse = '';
        let gotSessionId = false;
        const collector = governedProfile ? createGovernedNativeCollector(governedProfile) : null;
        let evidenceFailure = null;
        if (governedProfile) governedEvidenceHandlers.set(reqId, (event) => {
          try { collector.observe(event); } catch (error) { evidenceFailure ??= normalizeGovernedAnswerFailure(error, 'native_event_grammar'); }
        });
        if (opts.abortSignal) {
          if (opts.abortSignal.aborted) throw new Error('Codex query cancelled');
          abortHandler = () => { void cleanup(new Error('Codex query cancelled')).catch(() => {}); };
          opts.abortSignal.addEventListener('abort', abortHandler, { once: true });
        }

        // Register event handler for this request
        eventHandlers.set(reqId, (eventParams) => {
          const msg = eventParams.msg;

          // Extract session_id from session_configured event
          if (!governedProfile && msg.type === 'session_configured' && msg.session_id && !gotSessionId) {
            session.setConversationId(msg.session_id);
            gotSessionId = true;
          }

          // Collect agent messages
          if (msg.type === 'raw_response_item' && msg.item?.role === 'assistant') {
            const content = msg.item.content;
            if (Array.isArray(content)) {
              for (const part of content) {
                if (part.type === 'text' && part.text) {
                  fullResponse += part.text;
                }
              }
            }
          }
        });

        // Call the tool
        const dispatch = governedProfile ? governedCodexDispatch(toolArgs.prompt) : null;
        const resultPromise = sendRequest('tools/call', {
          name: toolName,
          arguments: toolArgs
        });

        // Wait for result
        let result;
        try { result = await resultPromise; }
        catch (error) {
          throw governedProfile
            ? governedAnswerFailure(evidenceFailure ? 'unknown' : 'provider_engine')
            : error;
        }

        // Clean up event handler
        eventHandlers.delete(reqId);
        governedEvidenceHandlers.delete(reqId);
        let attestation = null;
        if (governedProfile) {
          if (evidenceFailure) throw evidenceFailure;
          let collected, internal;
          try {
            collected = collector.evidence();
          } catch (error) {
            throw normalizeGovernedAnswerFailure(error, 'native_event_grammar', 'live_envelope_session');
          }
          try {
            internal = attestGovernedCodexSession({ profile: governedProfile,
              events: governedProfile.version === 'probe.governed-codex-profile/v2'
                ? [collected.sessionEvent, collected.capabilities.nativeTools] : [collected.sessionEvent] });
          } catch (error) {
            throw normalizeGovernedAnswerFailure(error, 'native_event_grammar', 'live_envelope_session', 'attestation');
          }
          attestation = hasInvocationDigest
            ? externalBoundReceipt(internal, dispatch, invocationDigest, {
              nativeCallCount: collected.capabilities.nativeTools.total,
              probeMcpCallCount: collected.capabilities.probeMcpCallCount,
            })
            : externalReceipt(internal, {
              nativeCallCount: collected.capabilities.nativeTools.total,
              probeMcpCallCount: collected.capabilities.probeMcpCallCount,
            });
          session.setConversationId(collected.sessionEvent.params.msg.session_id);
        }

        // Parse result
        if (result && result.content && Array.isArray(result.content)) {
          for (const item of result.content) {
            if (item.type === 'text' && item.text) {
              yield {
                type: 'text',
                content: item.text
              };
              fullResponse = item.text; // Use final result if available
            }
          }
        }

        // If we got a response from events but not from result, yield it
        if (fullResponse && (!result.content || result.content.length === 0)) {
          yield {
            type: 'text',
            content: fullResponse
          };
        }

        if (governedProfile?.version === 'probe.governed-codex-profile/v2') {
          yield { type: 'toolBatch', total: attestation.observed.nativeTools.total,
            tools: attestation.observed.nativeTools.tools.map((item) => ({ ...item })) };
        }

        session.incrementMessageCount();

        yield {
          type: 'metadata',
          data: governedProfile ? { attestation } : {
            sessionId: session.id,
            conversationId: session.conversationId,
            messageCount: session.messageCount
          }
        };

      } catch (error) {
        if (governedProfile) error = normalizeGovernedAnswerFailure(error, 'unknown');
        queryError = error;
        if (debug) {
          console.error('[DEBUG] Codex query error:', error);
        }
        yield {
          type: 'error',
          error: error
        };
      } finally {
        if (opts.abortSignal && abortHandler) opts.abortSignal.removeEventListener('abort', abortHandler);
        if (governedProfile) {
          try { await cleanup(); }
          catch (cleanupError) { if (!queryError) throw cleanupError; }
        }
      }
    },

    /**
     * Get session info
     */
    getSession() {
      return session.getInfo();
    },

    /**
     * Clean up resources
     */
    async close() {
      await cleanup();
    }
  };
}

/**
 * Combine prompts intelligently
 */
function combinePrompts(systemPrompt, customPrompt) {
  if (!systemPrompt && customPrompt) {
    return customPrompt;
  }

  if (systemPrompt && customPrompt) {
    return systemPrompt + '\n\n## Additional Instructions\n' + customPrompt;
  }

  return systemPrompt || '';
}
