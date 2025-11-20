/**
 * OpenAI Codex Engine using MCP server approach with event streaming
 * Runs 'codex mcp-server' and handles codex/event notifications
 */

import { spawn } from 'child_process';
import { randomBytes } from 'crypto';
import { createInterface } from 'readline';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';

/**
 * Session manager for Codex conversations
 */
class CodexSession {
  constructor(id, debug = false) {
    this.id = id;
    this.conversationId = null;  // Codex session_id for resumption
    this.messageCount = 0;
    this.debug = debug;
  }

  setConversationId(conversationId) {
    this.conversationId = conversationId;
    if (this.debug) {
      console.log(`[Session ${this.id}] Codex Conversation ID: ${conversationId}`);
    }
  }

  incrementMessageCount() {
    this.messageCount++;
  }
}

/**
 * Codex Engine using MCP Server with event streaming
 */
export async function createCodexEngine(options = {}) {
  const { agent, systemPrompt, customPrompt, debug, sessionId, allowedTools, model } = options;

  const session = new CodexSession(
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
        const { resolve, reject } = pendingRequests.get(message.id);
        pendingRequests.delete(message.id);

        if (message.error) {
          reject(new Error(message.error.message || JSON.stringify(message.error)));
        } else {
          resolve(message.result);
        }
      }

      // Handle notifications (codex/event)
      if (message.method === 'codex/event' && message.params) {
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

  // Handle stderr
  if (debug) {
    codexProcess.stderr.on('data', (data) => {
      console.error('[CODEX STDERR]', data.toString());
    });
  }

  // Send JSON-RPC request
  function sendRequest(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++requestId;
      const request = {
        jsonrpc: '2.0',
        id,
        method,
        params
      };

      pendingRequests.set(id, { resolve, reject });

      // Timeout after 10 minutes
      setTimeout(() => {
        if (pendingRequests.has(id)) {
          pendingRequests.delete(id);
          reject(new Error(`Request ${method} timed out after 10 minutes`));
        }
      }, 600000);

      codexProcess.stdin.write(JSON.stringify(request) + '\n');
    });
  }

  // Initialize MCP connection
  await sendRequest('initialize', {
    protocolVersion: '2024-11-05',
    capabilities: { tools: {} },
    clientInfo: {
      name: 'probe-codex-client',
      version: '1.0.0'
    }
  });

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

      // Build arguments
      const toolArgs = { prompt: finalPrompt };

      if (isFollowUp) {
        toolArgs.conversationId = session.conversationId;
        if (debug) {
          console.log(`[DEBUG] Follow-up with conversationId: ${session.conversationId}`);
        }
      } else {
        if (model) {
          toolArgs.model = model;
        }
        if (mcpServerUrl && mcpServerName) {
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

      try {
        const reqId = requestId + 1;
        let fullResponse = '';
        let gotSessionId = false;

        // Register event handler for this request
        const eventPromise = new Promise((resolve) => {
          eventHandlers.set(reqId, (eventParams) => {
            const msg = eventParams.msg;

            // Extract session_id from session_configured event
            if (msg.type === 'session_configured' && msg.session_id && !gotSessionId) {
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

          // Mark as resolved when we're done
          setTimeout(() => {
            eventHandlers.delete(reqId);
            resolve();
          }, 600000); // 10 min timeout
        });

        // Call the tool
        const resultPromise = sendRequest('tools/call', {
          name: toolName,
          arguments: toolArgs
        });

        // Wait for result
        const result = await resultPromise;

        // Clean up event handler
        eventHandlers.delete(reqId);

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
          data: {
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
      }
    },

    /**
     * Get session info
     */
    getSession() {
      return {
        id: session.id,
        conversationId: session.conversationId,
        messageCount: session.messageCount
      };
    },

    /**
     * Clean up resources
     */
    async close() {
      try {
        // Close readline interface first to remove event listeners
        if (stdoutReader) {
          stdoutReader.close();
          if (debug) {
            console.log('[DEBUG] Closed stdout reader');
          }
        }

        // Clear all pending requests and event handlers
        pendingRequests.clear();
        eventHandlers.clear();

        // Kill Codex process
        if (codexProcess && !codexProcess.killed) {
          codexProcess.kill();
          if (debug) {
            console.log('[DEBUG] Killed Codex MCP server process');
          }
        }

        // Stop Probe MCP server
        if (mcpServer) {
          await mcpServer.stop();
          if (debug) {
            console.log('[DEBUG] Stopped Probe MCP server');
          }
        }

        if (debug) {
          console.log('[DEBUG] Engine closed, session:', session.id);
        }
      } catch (error) {
        if (debug) {
          console.error('[DEBUG] Error during cleanup:', error.message);
        }
      }
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
