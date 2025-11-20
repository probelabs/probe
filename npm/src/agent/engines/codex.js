/**
 * OpenAI Codex Engine with session management and better streaming
 * Uses the actual Codex CLI command structure with --config for MCP
 */

import { spawn } from 'child_process';
import { randomBytes } from 'crypto';
import { EventEmitter } from 'events';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';

/**
 * Session manager for Codex conversations
 */
class CodexSession {
  constructor(id, debug = false) {
    this.id = id;
    this.threadId = null;
    this.messageCount = 0;
    this.debug = debug;
  }

  /**
   * Update session with Codex's thread ID
   */
  setThreadId(threadId) {
    this.threadId = threadId;
    if (this.debug) {
      console.log(`[Session ${this.id}] Thread ID: ${threadId}`);
    }
  }

  incrementMessageCount() {
    this.messageCount++;
  }
}

/**
 * Codex Engine
 */
export async function createCodexEngine(options = {}) {
  const { agent, systemPrompt, customPrompt, debug, sessionId, allowedTools } = options;

  // Create or reuse session
  const session = new CodexSession(
    sessionId || randomBytes(8).toString('hex'),
    debug
  );

  // Start built-in MCP server with ephemeral port
  let mcpServer = null;
  let mcpServerUrl = null;
  let mcpServerName = null;

  if (agent) {
    mcpServer = new BuiltInMCPServer(agent, {
      port: 0,  // Ephemeral port
      host: '127.0.0.1',
      debug: debug
    });

    const { host, port } = await mcpServer.start();
    mcpServerUrl = `http://${host}:${port}/sse`;
    mcpServerName = `probe_${session.id}`;

    if (debug) {
      console.log('[DEBUG] Built-in MCP server started');
      console.log('[DEBUG] MCP SSE URL:', mcpServerUrl);
      console.log('[DEBUG] MCP server name:', mcpServerName);
    }
  }

  if (debug) {
    console.log('[DEBUG] Codex Engine initialized');
    console.log('[DEBUG] Session:', session.id);
  }

  // Combine prompts with system prompt
  const fullPrompt = combinePrompts(systemPrompt, customPrompt, agent);

  return {
    sessionId: session.id,
    session,
    mcpServerUrl,  // Store for --config
    mcpServerName,

    /**
     * Query Codex with advanced streaming
     */
    async *query(prompt, opts = {}) {
      const emitter = new EventEmitter();
      let buffer = '';
      let processEnded = false;
      let fullResponse = '';

      // Build final prompt with system instructions
      let finalPrompt = fullPrompt ? `${fullPrompt}\n\n${prompt}` : prompt;

      // For schema mode, append schema requirement
      if (opts.schema) {
        finalPrompt = `${finalPrompt}\n\nPlease provide your response in the following JSON format:\n${opts.schema}`;
      }

      // Build command arguments with MCP configuration
      const args = buildCodexArgs({
        prompt: finalPrompt,
        session,
        debug,
        allowedTools,
        mcpServerUrl,
        mcpServerName
      });

      if (debug) {
        console.log('[DEBUG] Executing: codex exec', args.slice(0, 10).join(' '), '...');
      }

      // Initialize tool collector for batch emission
      const toolCollector = [];

      // Spawn codex exec
      const proc = spawn('codex', ['exec', ...args], {
        env: { ...process.env },
        stdio: ['pipe', 'pipe', 'pipe']
      });

      // Send prompt via stdin
      proc.stdin.write(finalPrompt);
      proc.stdin.end();

      // Handle stdout - JSON lines
      proc.stdout.on('data', (data) => {
        buffer += data.toString();
        processJsonBuffer(buffer, emitter, session, debug, toolCollector);

        // Keep only incomplete line in buffer
        const lines = buffer.split('\n');
        buffer = lines[lines.length - 1] || '';
      });

      // Handle stderr
      proc.stderr.on('data', (data) => {
        const stderr = data.toString();
        if (debug) {
          console.error('[STDERR]', stderr);
        }

        // Check for important errors
        if (stderr.includes('command not found')) {
          emitter.emit('error', new Error('Codex CLI not found. Please install it first.'));
        }
      });

      // Handle process end
      proc.on('close', (code) => {
        processEnded = true;
        if (code !== 0 && debug) {
          console.log(`[DEBUG] Process exited with code ${code}`);
        }

        // Process any remaining buffer
        if (buffer.trim()) {
          processJsonBuffer(buffer, emitter, session, debug, toolCollector);
        }

        // Emit collected tool events as batch
        if (toolCollector.length > 0) {
          emitter.emit('toolBatch', {
            tools: toolCollector,
            timestamp: new Date().toISOString()
          });

          if (debug) {
            console.log(`[DEBUG] Emitting batch of ${toolCollector.length} tool events`);
          }
        }

        emitter.emit('end');
      });

      proc.on('error', (error) => {
        emitter.emit('error', error);
      });

      // Stream generator
      const messageQueue = [];
      let resolver = null;

      emitter.on('message', (msg) => {
        messageQueue.push(msg);
        if (resolver) {
          resolver();
          resolver = null;
        }
      });

      emitter.on('toolBatch', (batch) => {
        messageQueue.push({ type: 'toolBatch', ...batch });
        if (resolver) {
          resolver();
          resolver = null;
        }
      });

      emitter.on('end', () => {
        processEnded = true;
        if (resolver) {
          resolver();
          resolver = null;
        }
      });

      emitter.on('error', (error) => {
        messageQueue.push({ type: 'error', error });
        if (resolver) {
          resolver();
          resolver = null;
        }
      });

      // Process messages
      while (!processEnded || messageQueue.length > 0) {
        if (messageQueue.length > 0) {
          const msg = messageQueue.shift();

          if (msg.type === 'text') {
            yield { type: 'text', content: msg.content };
            fullResponse += msg.content;
          } else if (msg.type === 'toolBatch') {
            // Pass through the tool batch for ProbeAgent to emit
            yield { type: 'toolBatch', tools: msg.tools, timestamp: msg.timestamp };
          } else if (msg.type === 'thread_started') {
            // Thread started event
            if (debug) {
              console.log('[DEBUG] Thread started:', msg.threadId);
            }
          } else if (msg.type === 'error') {
            yield { type: 'error', error: msg.error };
            break;
          }
        } else if (!processEnded) {
          // Wait for more messages
          await new Promise(resolve => {
            resolver = resolve;
          });
        }
      }

      // Increment message count
      session.incrementMessageCount();

      // Return session info for potential resume
      yield {
        type: 'metadata',
        data: {
          sessionId: session.id,
          threadId: session.threadId,
          messageCount: session.messageCount
        }
      };
    },

    /**
     * Get session info
     */
    getSession() {
      return {
        id: session.id,
        threadId: session.threadId,
        messageCount: session.messageCount
      };
    },

    /**
     * Clean up - MUST be called to stop MCP server and clean resources
     */
    async close() {
      try {
        // Stop built-in MCP server
        if (mcpServer) {
          await mcpServer.stop();
          if (debug) {
            console.log('[DEBUG] Built-in MCP server stopped');
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
 * Process JSON buffer and emit messages
 * Codex CLI emits JSONL events like:
 * {"type":"thread.started","thread_id":"..."}
 * {"type":"turn.started"}
 * {"type":"item.completed","item":{"id":"...","type":"reasoning","text":"..."}}
 * {"type":"item.completed","item":{"id":"...","type":"agent_message","text":"..."}}
 * {"type":"turn.completed","usage":{...}}
 */
function processJsonBuffer(buffer, emitter, session, debug, toolCollector = null) {
  const lines = buffer.split('\n');

  for (const line of lines) {
    if (!line.trim()) continue;

    try {
      const event = JSON.parse(line);

      switch (event.type) {
        case 'thread.started':
          session.setThreadId(event.thread_id);
          emitter.emit('message', { type: 'thread_started', threadId: event.thread_id });
          break;

        case 'turn.started':
          if (debug) {
            console.log('[DEBUG] Turn started');
          }
          break;

        case 'item.completed':
          if (event.item) {
            // Handle different item types
            if (event.item.type === 'agent_message') {
              // Main agent response
              emitter.emit('message', { type: 'text', content: event.item.text });
            } else if (event.item.type === 'reasoning') {
              // Reasoning step - could extract tool usage here if needed
              if (debug) {
                console.log('[DEBUG] Reasoning:', event.item.text?.substring(0, 100));
              }
            } else if (event.item.type === 'tool_call') {
              // Tool call detected
              if (toolCollector) {
                toolCollector.push({
                  timestamp: new Date().toISOString(),
                  name: event.item.name || 'unknown',
                  args: event.item.arguments || {},
                  id: event.item.id,
                  status: 'started'
                });
              }
              if (debug) {
                console.log('[DEBUG] Tool call:', event.item.name);
              }
            } else if (event.item.type === 'tool_result') {
              // Tool result
              if (toolCollector && event.item.call_id) {
                const toolCall = toolCollector.find(t => t.id === event.item.call_id);
                if (toolCall) {
                  toolCall.status = 'completed';
                  toolCall.resultPreview = event.item.content ?
                    (typeof event.item.content === 'string' ?
                      event.item.content.substring(0, 200) :
                      JSON.stringify(event.item.content).substring(0, 200)) + '...' :
                    'No Result';
                }
              }
              if (debug) {
                console.log('[DEBUG] Tool result for:', event.item.call_id);
              }
            }
          }
          break;

        case 'turn.completed':
          if (debug && event.usage) {
            console.log('[DEBUG] Turn completed. Tokens:', event.usage);
          }
          break;

        default:
          if (debug) {
            console.log('[DEBUG] Unknown event type:', event.type);
          }
      }
    } catch (e) {
      // Not valid JSON, might be partial
      if (debug && line.trim()) {
        console.log('[DEBUG] Non-JSON output:', line.substring(0, 100));
      }
    }
  }
}

/**
 * Build codex exec command arguments
 * Uses --config to dynamically add MCP server configuration
 */
function buildCodexArgs({ prompt, session, debug, allowedTools, mcpServerUrl, mcpServerName }) {
  const args = [
    '--json',  // Enable JSON output
  ];

  // Add MCP server configuration dynamically using --config
  // Codex supports HTTP/SSE MCP servers via URL
  // Format: -c 'mcp_servers.<name>.url="http://..."'
  if (mcpServerUrl && mcpServerName) {
    // Add HTTP MCP server configuration via --config parameter
    args.push('-c', `mcp_servers.${mcpServerName}.url="${mcpServerUrl}"`);

    if (debug) {
      console.log(`[DEBUG] Configured HTTP MCP server via --config: ${mcpServerName} -> ${mcpServerUrl}`);
    }
  }

  // Read prompt from stdin
  args.push('-');

  return args;
}

/**
 * Combine prompts intelligently
 */
function combinePrompts(systemPrompt, customPrompt, agent) {
  // For Codex, system instructions are prepended to the user message
  // since there's no separate --system-prompt flag

  // If only customPrompt is provided (no systemPrompt), use it as the main prompt
  if (!systemPrompt && customPrompt) {
    return customPrompt;
  }

  // If systemPrompt is provided, it's already complete from getCodexNativeSystemPrompt
  // Just add customPrompt if available
  if (systemPrompt && customPrompt) {
    return systemPrompt + '\n\n## Additional Instructions\n' + customPrompt;
  }

  // Return systemPrompt as-is if no customPrompt
  return systemPrompt || '';
}
