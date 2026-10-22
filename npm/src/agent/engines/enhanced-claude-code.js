/**
 * Enhanced Claude Code Engine with session management and better streaming
 */

import { spawn } from 'child_process';
import { randomBytes } from 'crypto';
import fs from 'fs/promises';
import { existsSync, readFileSync } from 'fs';
import path from 'path';
import os from 'os';
import { EventEmitter } from 'events';
import { BuiltInMCPServer } from '../mcp/built-in-server.js';
import { Session } from '../shared/Session.js';

/** Claude Code exits 1 once piped stdin exceeds this many characters (10MB). */
export const CLAUDE_MAX_STDIN_CHARS = 10485760;
/** System prompts above this size go through --system-prompt-file (Linux MAX_ARG_STRLEN is 128 KiB). */
export const SYSTEM_PROMPT_FILE_THRESHOLD_BYTES = 32 * 1024;
const STDERR_TAIL_CHARS = 2000;
const CLAUDE_NOT_FOUND_MESSAGE = 'Claude Code not found. Please install it first.';

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

      // Claude Code rejects piped stdin over 10MB ("piped stdin input exceeds
      // 10MB") with exit 1. Fail loudly up front instead of spawning a process
      // whose failure would otherwise look like an empty answer.
      if (finalPrompt.length > CLAUDE_MAX_STDIN_CHARS) {
        yield {
          type: 'error',
          error: new Error(
            `Prompt is ${finalPrompt.length} characters; Claude Code accepts at most ` +
            `${CLAUDE_MAX_STDIN_CHARS} characters (10MB) of piped stdin input. ` +
            'Reduce the prompt or pass large content as a file path.'
          )
        };
        return;
      }

      // Resolve how to launch the CLI (direct argv on Unix; claude.exe, node
      // <cli.js> or a quoted cmd.exe call for the npm shim on Windows).
      const launch = resolveClaudeLaunch();
      if (!launch) {
        yield { type: 'error', error: new Error(CLAUDE_NOT_FOUND_MESSAGE) };
        return;
      }

      let systemPromptFile = null;
      try {
        // A long system prompt goes through a private temp file so it never
        // hits the per-argument limit (128 KiB on Linux); the cmd.exe route
        // always uses the file so no newline or % reaches the command line.
        if (fullSystemPrompt && (launch.viaCmd || Buffer.byteLength(fullSystemPrompt, 'utf8') > SYSTEM_PROMPT_FILE_THRESHOLD_BYTES)) {
          systemPromptFile = await writeSystemPromptFile(fullSystemPrompt, session.id);
        }

        // Build command arguments
        const args = buildClaudeArgs({
          systemPrompt: fullSystemPrompt,
          systemPromptFile,
          mcpConfigPath,
          session,
          debug,
          allowedTools: allowedTools || opts.allowedTools  // Support tool filtering
        });

        if (debug) {
          console.log('[DEBUG] Executing: claude', args.join(' '));
          console.log('[DEBUG] Prompt bytes (sent on stdin):', Buffer.byteLength(finalPrompt, 'utf8'));
        }

        // Initialize tool collector for batch emission
        const toolCollector = [];

        // Spawn `claude` directly with an argv array (no `sh -c`, no shell
        // escaping) and deliver the prompt on stdin. The prompt used to be a
        // positional argument inside an `echo "" | claude ...` shell string, so a
        // large prompt overflowed ARG_MAX (MAX_ARG_STRLEN on Linux) and the spawn
        // failed with E2BIG. `claude -p` reads its prompt from stdin when no
        // positional prompt is given, and closing stdin after writing also keeps
        // the CLI from waiting on an open stdin (the reason for the old echo pipe).
        const { proc } = spawnClaude(args, finalPrompt, { debug, launch });

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

        // Outcome tracking: a non-zero exit with no answer becomes an error
        let sawText = false;
        let errored = false;

        // Handle stderr (keep a short tail for error reports)
        let stderrTail = '';
        proc.stderr.on('data', (data) => {
          const stderr = data.toString();
          stderrTail = (stderrTail + stderr).slice(-STDERR_TAIL_CHARS);
          if (debug) {
            console.error('[STDERR]', stderr);
          }

          // Check for important errors
          if (stderr.includes('command not found')) {
            emitter.emit('error', new Error(CLAUDE_NOT_FOUND_MESSAGE));
          }
        });

        // Handle process end
        proc.on('close', (code, signal) => {
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

          // A failed run with no answer is an error, not an empty answer
          // (e.g. "piped stdin input exceeds 10MB", auth failures).
          if (code !== 0 && !sawText && !errored) {
            const status = code === null ? `signal ${signal}` : `code ${code}`;
            const detail = stderrTail.trim();
            emitter.emit('error', new Error(
              `Claude Code exited with ${status} and no result` + (detail ? `: ${detail}` : '')
            ));
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
          // On Unix the PATH lookup happens in spawn, so ENOENT means the CLI
          // is missing. On Windows the lookup already ran in
          // resolveClaudeLaunch, so a spawn error is reported as-is.
          if (error && error.code === 'ENOENT' && launch.platform !== 'win32') {
            emitter.emit('error', new Error(CLAUDE_NOT_FOUND_MESSAGE));
            return;
          }
          emitter.emit('error', error);
        });

        // Stream generator
        const messageQueue = [];
        let resolver = null;

        emitter.on('message', (msg) => {
          if (msg.type === 'text') sawText = true;
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
          errored = true;
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
              } catch (error) {
                result = `Tool error: ${error.message}`;
              } finally {
                // Always clear timeout to prevent memory leaks
                clearTimeout(toolTimeoutId);
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
      } finally {
        if (systemPromptFile) {
          await fs.unlink(systemPromptFile).catch(() => {});
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
export function buildClaudeArgs({ systemPrompt, systemPromptFile, mcpConfigPath, session, debug, allowedTools }) {
  const args = [
    '-p',  // Short form of --print; the prompt itself is written to stdin
    '--output-format', 'json'
  ];

  // Add session resume if available
  const resumeArgs = session.getResumeArgs();
  if (resumeArgs.length > 0) {
    args.push(...resumeArgs);
  }

  // Add system prompt (from a private temp file when it is large)
  if (systemPromptFile) {
    args.push('--system-prompt-file', systemPromptFile);
  } else if (systemPrompt) {
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
 * Decide how to start the Claude Code CLI without a shell.
 *
 * - Unix: `claude` with the args array; spawn does the PATH lookup.
 * - Windows: npm installs `claude.cmd` (plus an extensionless sh shim), which
 *   spawn cannot run without a shell (EINVAL / ENOENT). Search PATH for, in
 *   order: `claude.exe`; an npm shim whose package `bin` entry can be run
 *   directly (`node <cli.js>` or the native .exe); `claude.cmd` through
 *   `cmd.exe /d /s /c` with per-argument quoting. Returns null when nothing is
 *   found, which the caller reports as "Claude Code not found".
 *
 * Exported for tests; `platform`, `env`, `fileExists`, `readFile` and
 * `execPath` are injectable.
 *
 * @returns {null | {platform: string, command: string, prefixArgs: string[], viaCmd: boolean}}
 */
export function resolveClaudeLaunch({
  platform = process.platform,
  env = process.env,
  fileExists = existsSync,
  readFile = (file) => readFileSync(file, 'utf8'),
  execPath = process.execPath
} = {}) {
  if (platform !== 'win32') {
    return { platform, command: 'claude', prefixArgs: [], viaCmd: false };
  }

  const win = path.win32;
  const pathKey = Object.keys(env).find(k => k.toUpperCase() === 'PATH');
  const dirs = (pathKey ? env[pathKey] : '').split(';').map(d => d.trim().replace(/^"(.*)"$/, '$1')).filter(Boolean);

  for (const dir of dirs) {
    const exe = win.join(dir, 'claude.exe');
    if (fileExists(exe)) {
      return { platform, command: exe, prefixArgs: [], viaCmd: false };
    }
  }

  for (const dir of dirs) {
    const shim = win.join(dir, 'claude.cmd');
    if (!fileExists(shim)) continue;

    // npm global layout: <prefix>\claude.cmd + <prefix>\node_modules\@anthropic-ai\claude-code
    const pkgDir = win.join(dir, 'node_modules', '@anthropic-ai', 'claude-code');
    const entry = resolvePackageBin(pkgDir, { fileExists, readFile });
    if (entry) {
      if (/\.(c|m)?js$/i.test(entry)) {
        return { platform, command: execPath, prefixArgs: [entry], viaCmd: false };
      }
      if (/\.exe$/i.test(entry)) {
        return { platform, command: entry, prefixArgs: [], viaCmd: false };
      }
    }

    return {
      platform,
      command: env.ComSpec || env.COMSPEC || 'cmd.exe',
      prefixArgs: [],
      viaCmd: true,
      cmdShim: shim
    };
  }

  return null;
}

function resolvePackageBin(pkgDir, { fileExists, readFile }) {
  const win = path.win32;
  const pkgJson = win.join(pkgDir, 'package.json');
  if (fileExists(pkgJson)) {
    try {
      const pkg = JSON.parse(readFile(pkgJson));
      const bin = typeof pkg.bin === 'string' ? pkg.bin : pkg.bin && pkg.bin.claude;
      if (typeof bin === 'string') {
        const entry = win.join(pkgDir, bin);
        if (fileExists(entry)) return entry;
      }
    } catch {
      // fall through to the conventional entry point
    }
  }
  const cli = win.join(pkgDir, 'cli.js');
  return fileExists(cli) ? cli : null;
}

// cmd.exe metacharacters, escaped with ^ (same approach as cross-spawn).
const CMD_META_CHARS = /([()\][%!^"`<>&|;, *?])/g;

/**
 * Quote one argument for `cmd.exe /d /s /c "<shim> args..."` running a .cmd
 * shim. MSVCRT-style quoting, then cmd metacharacters (including % and ")
 * are ^-escaped twice: once for cmd.exe, once for the batch file re-parse.
 * Newlines cannot be carried through cmd.exe at all, so they are rejected
 * (the system prompt goes through --system-prompt-file on this route and
 * the user prompt through stdin).
 */
export function quoteCmdArg(arg) {
  if (/[\r\n]/.test(arg)) {
    throw new TypeError('Argument contains a newline and cannot be passed through cmd.exe');
  }
  let quoted = arg.replace(/(\\*)"/g, '$1$1\\"');
  quoted = quoted.replace(/(\\*)$/, '$1$1');
  quoted = `"${quoted}"`;
  quoted = quoted.replace(CMD_META_CHARS, '^$1');
  return quoted.replace(CMD_META_CHARS, '^$1');
}

/** Build the spawn file + argv + options for a resolved launch (exported for tests). */
export function buildClaudeSpawn(launch, args) {
  if (launch.viaCmd) {
    const command = launch.cmdShim.replace(CMD_META_CHARS, '^$1');
    const line = [command, ...args.map(quoteCmdArg)].join(' ');
    return {
      file: launch.command,
      argv: ['/d', '/s', '/c', `"${line}"`],
      options: { shell: false, windowsVerbatimArguments: true, windowsHide: true }
    };
  }
  return {
    file: launch.command,
    argv: [...launch.prefixArgs, ...args],
    options: launch.platform === 'win32' ? { shell: false, windowsHide: true } : {}
  };
}

/**
 * Write a large system prompt to a private (0600) temp file for
 * --system-prompt-file. The caller deletes it when the query finishes.
 */
async function writeSystemPromptFile(systemPrompt, sessionId) {
  const file = path.join(os.tmpdir(), `probe-claude-system-${sessionId}-${randomBytes(6).toString('hex')}.txt`);
  await fs.writeFile(file, systemPrompt, { encoding: 'utf8', mode: 0o600, flag: 'wx' });
  return file;
}

/**
 * Spawn the Claude Code CLI with an argv array and write the prompt to stdin.
 *
 * Exported for tests. Never throws on a broken stdin pipe: if `claude` exits
 * (or fails to start) before reading the whole prompt, the resulting EPIPE /
 * ERR_STREAM_DESTROYED is swallowed here and the process `close`/`error`
 * events carry the outcome.
 *
 * @param {string[]} args - claude CLI arguments (see buildClaudeArgs)
 * @param {string} prompt - prompt text delivered on stdin
 * @param {{debug?: boolean, launch?: object, env?: object, spawnImpl?: Function}} [options]
 */
export function spawnClaude(args, prompt, { debug = false, launch, env, spawnImpl = spawn } = {}) {
  for (const arg of args) {
    if (typeof arg !== 'string') {
      throw new TypeError(`Invalid argument type: expected string, got ${typeof arg}`);
    }
  }

  const resolved = launch || resolveClaudeLaunch() || { platform: process.platform, command: 'claude', prefixArgs: [], viaCmd: false };
  const { file, argv, options } = buildClaudeSpawn(resolved, args);

  const proc = spawnImpl(file, argv, {
    ...options,
    env: env || { ...process.env, FORCE_COLOR: '0' },
    stdio: ['pipe', 'pipe', 'pipe']
  });

  const result = { proc, stdinError: null };

  proc.stdin.on('error', (error) => {
    // EPIPE / ERR_STREAM_DESTROYED: the child exited or never started before
    // consuming stdin. Not fatal on its own; exit code / spawn error decide.
    result.stdinError = error;
    if (debug) {
      console.log('[DEBUG] Claude stdin closed early:', error.code || error.message);
    }
  });

  proc.stdin.end(typeof prompt === 'string' ? prompt : String(prompt ?? ''));

  return result;
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