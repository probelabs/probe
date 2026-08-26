/**
 * OpenAI Codex Engine using MCP server approach with event streaming
 * Runs 'codex mcp-server' and handles codex/event notifications
 */

import { spawn } from 'child_process';
import { createHash, randomBytes } from 'crypto';
import { createInterface } from 'readline';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { Session } from '../shared/Session.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs, validateGovernedCodexProfile } from './governed-codex-profile.js';

function externalReceipt(attestation) {
  return {
    version: attestation.version, profileId: attestation.profileId,
    requested: { profileDigest: attestation.requested.profileDigest, cwdDigest: attestation.requested.cwdDigest, probeToolsDigest: attestation.requested.probeToolsDigest, model: attestation.requested.model, reasoningEffort: attestation.requested.reasoningEffort, sandbox: attestation.requested.sandbox, approvalPolicy: attestation.requested.approvalPolicy },
    observed: { source: attestation.observed.source, model: attestation.observed.model, modelProviderId: attestation.observed.modelProviderId, reasoningEffort: attestation.observed.reasoningEffort, approvalPolicy: attestation.observed.approvalPolicy, cwdDigest: attestation.observed.cwdDigest, permissionProfileDigest: attestation.observed.permissionProfileDigest, filesystem: attestation.observed.filesystem, network: attestation.observed.network },
    evidence: { eventCount: 1 }, usage: { status: 'unavailable' },
  };
}

function governedCodexDispatch(prompt) {
  const promptBytes = Buffer.byteLength(prompt, 'utf8');
  const byteLength = Buffer.alloc(8);
  byteLength.writeBigUInt64BE(BigInt(promptBytes));
  const promptDigest = `sha256:${createHash('sha256')
    .update('probe.governed-codex-dispatch/prompt/v1', 'utf8')
    .update(Buffer.from([0])).update(byteLength).update(prompt, 'utf8').digest('hex')}`;
  return Object.freeze({ source: 'probe-host-tools-call', tool: 'codex', promptDigest, promptBytes });
}

function externalBoundReceipt(internal, dispatch, invocationDigest) {
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
    stdio: ['pipe', 'pipe', 'pipe']
  });

  // Setup JSON-RPC communication
  let requestId = 0;
  const pendingRequests = new Map();
  const eventHandlers = new Map();
  let governedEvidenceHandler = null, governedQueryStarted = false, closePromise = null;
  const processClosed = new Promise((resolve) => codexProcess.once('close', resolve));

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
        if (governedEvidenceHandler && message.params.msg?.type === 'session_configured') governedEvidenceHandler(message);
        const requestId = message.params._meta?.requestId;
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
      rejectPending(reason); eventHandlers.clear(); governedEvidenceHandler = null;
      stdoutReader.close(); codexProcess.stdin.destroy();
      if (codexProcess.exitCode === null && !codexProcess.killed) codexProcess.kill();
      await processClosed;
      if (mcpServer) await mcpServer.stop();
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

  const fullPrompt = combinePrompts(systemPrompt, customPrompt, agent);

  return {
    sessionId: session.id,
    session,

    /**
     * Query Codex via MCP protocol with event streaming
     */
    async *query(prompt, opts = {}) {
      // Build prompt
      let finalPrompt = prompt;
      if (!session.conversationId && fullPrompt) {
        finalPrompt = `${fullPrompt}\n\n${prompt}`;
      }

      const isFollowUp = session.conversationId !== null;
      const toolName = isFollowUp ? 'codex-reply' : 'codex';
      let abortHandler;

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
        const evidence = []; if (governedProfile) governedEvidenceHandler = (event) => evidence.push(event);
        if (opts.abortSignal) {
          if (opts.abortSignal.aborted) throw new Error('Codex query cancelled');
          abortHandler = () => cleanup(new Error('Codex query cancelled'));
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
        const result = await resultPromise;

        // Clean up event handler
        eventHandlers.delete(reqId);
        let attestation = null;
        if (governedProfile) {
          const internal = attestGovernedCodexSession({ profile: governedProfile, events: evidence });
          attestation = hasInvocationDigest
            ? externalBoundReceipt(internal, dispatch, invocationDigest)
            : externalReceipt(internal);
          session.setConversationId(evidence[0].params.msg.session_id);
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
        if (debug) {
          console.error('[DEBUG] Codex query error:', error);
        }
        yield {
          type: 'error',
          error: error
        };
      } finally {
        if (opts.abortSignal && abortHandler) opts.abortSignal.removeEventListener('abort', abortHandler);
        if (governedProfile) await cleanup();
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
function combinePrompts(systemPrompt, customPrompt, agent) {
  if (!systemPrompt && customPrompt) {
    return customPrompt;
  }

  if (systemPrompt && customPrompt) {
    return systemPrompt + '\n\n## Additional Instructions\n' + customPrompt;
  }

  return systemPrompt || '';
}
