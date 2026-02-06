/**
 * Enhanced Claude Code Engine with session management and better streaming
 */

import { spawn } from 'child_process';
import { randomBytes } from 'crypto';
import fs from 'fs/promises';
import path from 'path';
import os from 'os';
import { EventEmitter } from 'events';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { Session } from '../shared/Session.js';

/**
 * Enhanced Claude Code Engine
 */
export async function createEnhancedClaudeCLIEngine(options = {}) {
  const { agent, systemPrompt, customPrompt, debug, sessionId, allowedTools, timeout = 120000 } = options;

  // Create or reuse session
  const session = new Session(
    sessionId || randomBytes(8).toString('hex'),
    debug
  );

  // Start built-in MCP server with ephemeral port
  let mcpServer = null;
  let mcpConfigPath = null;

  if (agent) {
    mcpServer = new BuiltInMCPServer(agent, {
      port: 0,  // Ephemeral port
      host: '127.0.0.1',
      debug: debug
    });

    const { host, port } = await mcpServer.start();

    if (debug) {
      console.log('[DEBUG] Built-in MCP server started');
      console.log('[DEBUG] MCP URL:', `http://${host}:${port}/mcp`);
    }

    // Create MCP config for Claude Code to use
    // Note: Claude Code currently requires spawning a process, not HTTP transport
    // Keep built-in server running but also provide process-based config for CLI
    mcpConfigPath = path.join(os.tmpdir(), `probe-mcp-${session.id}.json`);
    const mcpConfig = {
      mcpServers: {
        probe: {
          command: 'node',
          args: [path.join(process.cwd(), 'mcp-probe-server.js')],
          env: {
            PROBE_WORKSPACE: process.cwd(),
            DEBUG: debug ? 'true' : 'false'
          }
        }
      }
    };

    await fs.writeFile(mcpConfigPath, JSON.stringify(mcpConfig, null, 2));
  }

  if (debug) {
    console.log('[DEBUG] Enhanced Claude Code Engine');
    console.log('[DEBUG] Session:', session.id);
    console.log('[DEBUG] MCP Config:', mcpConfigPath);
  }

  // Combine prompts
  const fullSystemPrompt = combinePrompts(systemPrompt, customPrompt, agent);

  return {
    sessionId: session.id,
    session,

    /**
     * Query Claude with advanced streaming
     */
    async *query(prompt, opts = {}) {
      const emitter = new EventEmitter();
      let buffer = '';
      let processEnded = false;
      let currentToolCall = null;
      let isSchemaMode = false;

      // Check if this is a schema reminder or validation request
      // In these cases, we treat Claude Code as a black box - just get the response
      if (opts.schema || prompt.includes('JSON schema') || prompt.includes('mermaid diagram')) {
        isSchemaMode = true;
        if (debug) {
          console.log('[DEBUG] Schema/validation mode - treating as black box');
        }
      }

      // For schema mode, append the schema requirement to the prompt
      let finalPrompt = prompt;
      if (opts.schema && isSchemaMode) {
        finalPrompt = `${prompt}\n\nPlease provide your response in the following JSON format:\n${opts.schema}`;
      }

      // Build command arguments
      const args = buildClaudeArgs({
        systemPrompt: fullSystemPrompt,
        mcpConfigPath,
        session,
        debug,
        prompt: finalPrompt,  // Use finalPrompt which may include schema
        allowedTools: allowedTools || opts.allowedTools  // Support tool filtering
      });

      if (debug) {
        console.log('[DEBUG] Executing: claude', args.join(' '));
      }

      // CRITICAL: Claude Code requires echo pipe to work when spawned from Node.js
      // Without it, the process hangs indefinitely waiting for stdin
      // This has been tested extensively - see CLAUDE_CLI_ECHO_REQUIREMENT.md
      // DO NOT REMOVE THE echo "" | PREFIX
      // SECURITY: Shell argument escaping using POSIX single-quote method
      // Single quotes in POSIX shells protect ALL metacharacters (;|&$`><*?) except single quote itself
      // The pattern 'text'\''more' correctly handles embedded quotes by:
      // 1. Closing the current quote with '
      // 2. Adding an escaped quote \'
      // 3. Opening a new quote with '
      // This is the standard POSIX-compliant method and is completely safe against injection
      const shellCmd = `echo "" | claude ${args.map(arg => {
        // Validate arg is a string (paranoid check)
        if (typeof arg !== 'string') {
          throw new TypeError(`Invalid argument type: expected string, got ${typeof arg}`);
        }
        // Escape single quotes using POSIX method: ' -> '\''
        const escaped = arg.replace(/'/g, "'\\''");
        // Wrap in single quotes for complete shell metacharacter protection
        return `'${escaped}'`;
      }).join(' ')}`;

      if (debug) {
        console.log('[DEBUG] Shell command length:', shellCmd.length);
        // Don't log full command if too long (e.g. system prompt)
        if (shellCmd.length < 500) {
          console.log('[DEBUG] Shell command:', shellCmd);
        } else {
          console.log('[DEBUG] Shell command (truncated):', shellCmd.substring(0, 200) + '...');
        }
      }

      // Initialize tool collector for batch emission
      const toolCollector = [];

      // Spawn using shell wrapper with echo pipe
      const proc = spawn('sh', ['-c', shellCmd], {
        env: { ...process.env, FORCE_COLOR: '0' },
        stdio: ['ignore', 'pipe', 'pipe']  // Ignore stdin since echo handles it
      });

      // Subprocess timeout handling
      let killed = false;
      let timeoutHandle;
      let sigkillHandle;

      if (timeout > 0) {
        timeoutHandle = setTimeout(() => {
          if (!killed) {
            killed = true;
            processEnded = true;
            proc.kill('SIGTERM');

            if (debug) {
              console.log(`[DEBUG] Process timed out after ${timeout}ms, sending SIGTERM`);
            }

            // Force kill after 5 seconds if still running
            sigkillHandle = setTimeout(() => {
              if (proc.exitCode === null) {
                proc.kill('SIGKILL');
                if (debug) {
                  console.log('[DEBUG] Process did not exit, sending SIGKILL');
                }
              }
            }, 5000);

            emitter.emit('error', new Error(`Claude CLI process timed out after ${timeout}ms`));
          }
        }, timeout);
      }

      // Handle stdout
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
          emitter.emit('error', new Error('Claude Code not found. Please install it first.'));
        }
      });

      // Handle process end
      proc.on('close', (code) => {
        // Clear the timeouts to prevent memory leaks
        if (timeoutHandle) {
          clearTimeout(timeoutHandle);
        }
        if (sigkillHandle) {
          clearTimeout(sigkillHandle);
        }

        processEnded = true;
        if (code !== 0 && debug) {
          console.log(`[DEBUG] Process exited with code ${code}`);
        }

        // If killed by timeout, the error was already emitted
        if (killed) {
          emitter.emit('end');
          return;
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
        // Clear the timeouts to prevent memory leaks
        if (timeoutHandle) {
          clearTimeout(timeoutHandle);
        }
        if (sigkillHandle) {
          clearTimeout(sigkillHandle);
        }
        processEnded = true;
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
          } else if (msg.type === 'tool_use') {
            // Start tool execution
            currentToolCall = msg;
            yield {
              type: 'text',
              content: `\n🔧 Using ${msg.name}: ${JSON.stringify(msg.input)}\n`
            };

            // Execute tool with timeout to prevent indefinite blocking
            const toolTimeout = 30000; // 30 seconds
            let toolTimeoutId;
            const timeoutPromise = new Promise((_, reject) => {
              toolTimeoutId = setTimeout(() => reject(new Error(`Tool ${msg.name} timed out after ${toolTimeout}ms`)), toolTimeout);
            });
            let result;
            try {
              result = await Promise.race([
                executeProbleTool(agent, msg.name, msg.input),
                timeoutPromise
              ]);
              clearTimeout(toolTimeoutId); // Clear timeout on success
            } catch (error) {
              clearTimeout(toolTimeoutId); // Clear timeout on error too
              result = `Tool error: ${error.message}`;
            }
            yield { type: 'text', content: `${result}\n` };
          } else if (msg.type === 'toolBatch') {
            // Pass through the tool batch for ProbeAgent to emit
            yield { type: 'toolBatch', tools: msg.tools, timestamp: msg.timestamp };
          } else if (msg.type === 'session_update') {
            // Session was updated with conversation ID
            if (debug) {
              console.log('[DEBUG] Session updated:', msg.conversationId);
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
          conversationId: session.conversationId,
          messageCount: session.messageCount
        }
      };
    },

    /**
     * Get session info
     */
    getSession() {
      return session.getInfo();
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

        // Remove temporary MCP config file
        if (mcpConfigPath) {
          await fs.unlink(mcpConfigPath).catch(() => {});
          if (debug) {
            console.log('[DEBUG] MCP config file removed');
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
 */
function processJsonBuffer(buffer, emitter, session, debug, toolCollector = null) {
  const lines = buffer.split('\n');

  for (const line of lines) {
    if (!line.trim()) continue;

    try {
      const parsed = JSON.parse(line);

      // Claude Code might return an array of messages
      const messages = Array.isArray(parsed) ? parsed : [parsed];

      for (const msg of messages) {

      switch (msg.type) {
        case 'result':
          // Claude Code returns a complete result object
          if (msg.result) {
            emitter.emit('message', { type: 'text', content: msg.result });
          }
          if (msg.session_id) {
            session.setConversationId(msg.session_id);
            emitter.emit('message', { type: 'session_update', conversationId: msg.session_id });
          }
          break;

        case 'conversation':
          session.setConversationId(msg.id);
          emitter.emit('message', { type: 'session_update', conversationId: msg.id });
          break;

        case 'text':
          if (msg.text) {
            emitter.emit('message', { type: 'text', content: msg.text });
          }
          break;

        case 'assistant':
          // Claude Code emits assistant messages when using internal agents/tools
          if (msg.message && msg.message.content) {
            // Extract text from the content array
            for (const content of msg.message.content) {
              if (content.type === 'text' && content.text) {
                emitter.emit('message', { type: 'text', content: content.text });
              } else if (content.type === 'tool_use') {
                // Collect tool call for batch emission
                if (toolCollector) {
                  toolCollector.push({
                    timestamp: new Date().toISOString(),
                    name: content.name,
                    args: content.input || {},
                    id: content.id,
                    status: 'started'
                  });
                }
                // Internal tool use - already handled by Claude Code
                if (debug) {
                  console.log('[DEBUG] Assistant internal tool use:', content.name);
                }
              }
            }
          }
          break;

        case 'tool_use':
          // Collect tool call for batch emission
          if (toolCollector) {
            toolCollector.push({
              timestamp: new Date().toISOString(),
              name: msg.name,
              args: msg.input || {},
              id: msg.id,
              status: 'started'
            });
          }
          emitter.emit('message', {
            type: 'tool_use',
            id: msg.id,
            name: msg.name,
            input: msg.input
          });
          break;

        case 'tool_result':
          // Mark tool as completed in collector
          if (toolCollector && msg.tool_use_id) {
            // Find the matching tool call and update its status
            const toolCall = toolCollector.find(t => t.id === msg.tool_use_id);
            if (toolCall) {
              toolCall.status = 'completed';
              toolCall.resultPreview = msg.content ?
                (typeof msg.content === 'string' ?
                  msg.content.substring(0, 200) :
                  JSON.stringify(msg.content).substring(0, 200)) + '...' :
                'No Result';
            }
          }
          // Tool results are handled internally
          if (debug) {
            console.log('[DEBUG] Tool result:', msg);
          }
          break;

        case 'error':
          emitter.emit('error', new Error(msg.message || 'Unknown error'));
          break;

        default:
          if (debug) {
            console.log('[DEBUG] Unknown message type:', msg.type);
            console.log('[DEBUG] Full message:', JSON.stringify(msg).substring(0, 200));
          }
      }
      } // Close inner for loop for messages array
    } catch (e) {
      // Not valid JSON, might be partial
      if (debug && line.trim()) {
        console.log('[DEBUG] Non-JSON output:', line);
      }
    }
  }
}

/**
 * Build claude command arguments
 */
function buildClaudeArgs({ systemPrompt, mcpConfigPath, session, debug, prompt, allowedTools }) {
  const args = [
    '-p',  // Short form of --print
    prompt,  // The prompt text goes right after -p
    '--output-format', 'json'
  ];

  // Add session resume if available
  const resumeArgs = session.getResumeArgs();
  if (resumeArgs.length > 0) {
    args.push(...resumeArgs);
  }

  // Add system prompt
  if (systemPrompt) {
    args.push('--system-prompt', systemPrompt);
  }

  // Add MCP config
  args.push('--mcp-config', mcpConfigPath);

  // Add allowed tools filter if specified
  // If no filter specified, allow all probe tools
  if (allowedTools && Array.isArray(allowedTools) && allowedTools.length > 0) {
    // Convert tool names to MCP format: mcp__probe__<toolname>
    const mcpTools = allowedTools.map(tool =>
      tool.startsWith('mcp__') ? tool : `mcp__probe__${tool}`
    ).join(',');
    args.push('--allowedTools', mcpTools);
  } else {
    // Default: allow all probe tools
    args.push('--allowedTools', 'mcp__probe__*');
  }

  // Add debug flag
  if (debug) {
    args.push('--verbose');
  }

  return args;
}

/**
 * Execute Probe tool through agent
 */
async function executeProbleTool(agent, toolName, params) {
  if (!agent || !agent.toolImplementations) {
    return 'Tool execution not available';
  }

  // Remove MCP prefix: mcp__probe__<toolname> -> <toolname>
  const name = toolName.replace(/^mcp__probe__/, '');
  const tool = agent.toolImplementations[name];

  if (!tool) {
    return `Unknown tool: ${name}`;
  }

  try {
    const result = await tool.execute(params);
    return typeof result === 'string' ? result : JSON.stringify(result, null, 2);
  } catch (error) {
    return `Tool error: ${error.message}`;
  }
}

// Old createEnhancedMCPConfig function removed - now using built-in MCP server

/**
 * Combine prompts intelligently
 */
function combinePrompts(systemPrompt, customPrompt, agent) {
  // For Claude Code, the systemPrompt already contains all necessary instructions
  // from getClaudeNativeSystemPrompt(), so we don't need to add a base prompt

  // If only customPrompt is provided (no systemPrompt), use it as the main prompt
  if (!systemPrompt && customPrompt) {
    return customPrompt;
  }

  // If systemPrompt is provided, it's already complete from getClaudeNativeSystemPrompt
  // Just add customPrompt if available
  if (systemPrompt && customPrompt) {
    return systemPrompt + '\n\n## Additional Instructions\n' + customPrompt;
  }

  // Return systemPrompt as-is if no customPrompt
  return systemPrompt || '';
}