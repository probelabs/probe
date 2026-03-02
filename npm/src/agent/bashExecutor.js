/**
 * Bash command executor with security and timeout controls
 * @module agent/bashExecutor
 */

import { spawn } from 'child_process';
import { resolve, join } from 'path';
import { existsSync } from 'fs';
import { parseCommandForExecution, isComplexCommand } from './bashCommandUtils.js';

// ─── Interactive Command Detection ─────────────────────────────────────────

/**
 * Split a command string by shell operators (&&, ||, |, ;) while respecting quotes.
 * Used for interactive command detection in complex pipelines.
 * @param {string} command - Command string to split
 * @returns {string[]} Array of individual command strings
 */
function splitCommandComponents(command) {
  const parts = [];
  let current = '';
  let inQuote = false;
  let quoteChar = '';

  for (let i = 0; i < command.length; i++) {
    const c = command[i];
    const next = command[i + 1] || '';

    // Handle escape sequences
    if (c === '\\' && !inQuote) {
      current += c + next;
      i++;
      continue;
    }
    if (inQuote && quoteChar === '"' && c === '\\' && next) {
      current += c + next;
      i++;
      continue;
    }

    // Track quotes
    if (!inQuote && (c === '"' || c === "'")) {
      inQuote = true;
      quoteChar = c;
      current += c;
      continue;
    }
    if (inQuote && c === quoteChar) {
      inQuote = false;
      current += c;
      continue;
    }

    // Split on operators outside quotes
    if (!inQuote) {
      if ((c === '&' && next === '&') || (c === '|' && next === '|')) {
        if (current.trim()) parts.push(current.trim());
        current = '';
        i++;
        continue;
      }
      if (c === '|' || c === ';') {
        if (current.trim()) parts.push(current.trim());
        current = '';
        continue;
      }
    }

    current += c;
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

/**
 * Check a single (non-compound) command for interactive behavior.
 * Strips leading env-var assignments (e.g. GIT_EDITOR=true) before checking.
 *
 * @param {string} command - Single command string
 * @returns {string|null} Error message with suggestion, or null if not interactive
 */
function checkSingleCommandInteractive(command) {
  let effective = command.trim();

  // Strip leading VAR=VALUE prefixes (e.g. "GIT_EDITOR=true git rebase --continue")
  while (/^\w+=\S*\s/.test(effective)) {
    effective = effective.replace(/^\w+=\S*\s+/, '');
  }

  const parts = effective.split(/\s+/);
  const base = parts[0];
  const args = parts.slice(1);

  // ── Interactive editors ──
  if (['vi', 'vim', 'nvim', 'nano', 'emacs', 'pico', 'joe', 'mcedit'].includes(base)) {
    return `'${base}' is an interactive editor and cannot run without a terminal. Use non-interactive file manipulation commands instead.`;
  }

  // ── Interactive pagers ──
  if (['less', 'more'].includes(base)) {
    return `'${base}' is an interactive pager. Use 'cat', 'head', or 'tail' instead.`;
  }

  // ── Git commands that open an editor ──
  if (base === 'git') {
    const sub = args[0];

    // git commit without -m / --message / -C / -c / --fixup / --squash / --no-edit
    if (sub === 'commit') {
      const hasNonInteractiveFlag = args.some(a =>
        a === '-m' || a.startsWith('--message') ||
        a === '-C' || a === '-c' ||
        a.startsWith('--fixup') || a.startsWith('--squash') ||
        a === '--allow-empty-message' || a === '--no-edit'
      );
      if (!hasNonInteractiveFlag) {
        return "Interactive command: 'git commit' opens an editor for the commit message. Use 'git commit -m \"your message\"' instead.";
      }
    }

    // git rebase --continue / --skip (opens editor for commit message)
    if (sub === 'rebase' && (args.includes('--continue') || args.includes('--skip'))) {
      return "Interactive command: 'git rebase --continue' opens an editor. Set environment variable GIT_EDITOR=true to accept default messages, e.g. pass env: {GIT_EDITOR: 'true'} or prepend GIT_EDITOR=true to the command.";
    }

    // git rebase -i / --interactive
    if (sub === 'rebase' && (args.includes('-i') || args.includes('--interactive'))) {
      return "Interactive command: 'git rebase -i' requires an interactive editor. Interactive rebase cannot run without a terminal.";
    }

    // git merge without --no-edit / --no-commit / --ff-only
    if (sub === 'merge' && !args.includes('--no-edit') && !args.includes('--no-commit') && !args.includes('--ff-only')) {
      return "Interactive command: 'git merge' may open an editor for the merge commit message. Add '--no-edit' to accept the default message.";
    }

    // git cherry-pick without --no-edit
    if (sub === 'cherry-pick' && !args.includes('--no-edit')) {
      return "Interactive command: 'git cherry-pick' may open an editor. Add '--no-edit' to accept the default message.";
    }

    // git revert without --no-edit
    if (sub === 'revert' && !args.includes('--no-edit')) {
      return "Interactive command: 'git revert' opens an editor. Add '--no-edit' to accept the default message.";
    }

    // git tag -a without -m
    if (sub === 'tag' && args.includes('-a') && !args.some(a => a === '-m' || a.startsWith('--message'))) {
      return "Interactive command: 'git tag -a' opens an editor for the tag message. Use 'git tag -a <name> -m \"message\"' instead.";
    }

    // git add -i / --interactive / -p / --patch
    if (sub === 'add' && (args.includes('-i') || args.includes('--interactive') || args.includes('-p') || args.includes('--patch'))) {
      return "Interactive command: 'git add -i/-p' requires interactive input. Use 'git add <files>' to stage specific files instead.";
    }
  }

  // ── Interactive REPLs (no arguments = interactive mode) ──
  if (['python', 'python3', 'node', 'irb', 'ghci', 'lua', 'R', 'ruby'].includes(base) && args.length === 0) {
    return `Interactive command: '${base}' without arguments starts an interactive REPL. Provide a script file or use '-c'/'--eval' for inline code.`;
  }

  // ── Database clients without query flag ──
  if (base === 'mysql' && !args.some(a => a === '-e' || a.startsWith('--execute'))) {
    return "Interactive command: 'mysql' without -e flag starts an interactive session. Use 'mysql -e \"SQL QUERY\"' instead.";
  }
  if (base === 'psql' && !args.some(a => a === '-c' || a.startsWith('--command') || a === '-f' || a.startsWith('--file'))) {
    return "Interactive command: 'psql' without -c flag starts an interactive session. Use 'psql -c \"SQL QUERY\"' instead.";
  }

  // ── Interactive TUI tools ──
  if (['top', 'htop', 'btop', 'nmon'].includes(base)) {
    return `Interactive command: '${base}' is an interactive TUI tool. Use 'ps aux' or 'top -b -n 1' for non-interactive process listing.`;
  }

  return null;
}

/**
 * Check if a command (simple or complex) would require interactive TTY input.
 * For complex commands (with &&, ||, |, ;), checks each component individually.
 *
 * @param {string} command - Full command string
 * @returns {string|null} Error message with suggestion for non-interactive alternative, or null if OK
 */
export function checkInteractiveCommand(command) {
  if (!command || typeof command !== 'string') return null;

  const components = splitCommandComponents(command.trim());
  for (const component of components) {
    const result = checkSingleCommandInteractive(component);
    if (result) return result;
  }
  return null;
}

/**
 * Execute a bash command with security controls
 * @param {string} command - Command to execute
 * @param {Object} options - Execution options
 * @param {string} [options.workingDirectory] - Working directory for command execution
 * @param {number} [options.timeout=120000] - Timeout in milliseconds
 * @param {Object} [options.env={}] - Additional environment variables
 * @param {number} [options.maxBuffer=10485760] - Maximum buffer size (10MB)
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Promise<Object>} Execution result
 */
export async function executeBashCommand(command, options = {}) {
  const {
    workingDirectory = process.cwd(),
    timeout = 120000, // 2 minutes default
    env = {},
    maxBuffer = 10 * 1024 * 1024, // 10MB
    debug = false
  } = options;

  // Validate working directory
  let cwd = workingDirectory;
  try {
    cwd = resolve(cwd);
    if (!existsSync(cwd)) {
      throw new Error(`Working directory does not exist: ${cwd}`);
    }
  } catch (error) {
    return {
      success: false,
      error: `Invalid working directory: ${error.message}`,
      stdout: '',
      stderr: '',
      exitCode: 1,
      command,
      workingDirectory: cwd,
      duration: 0
    };
  }

  const startTime = Date.now();

  // Check for interactive commands that would hang without a TTY
  const interactiveError = checkInteractiveCommand(command);
  if (interactiveError) {
    if (debug) {
      console.log(`[BashExecutor] Blocked interactive command: "${command}"`);
      console.log(`[BashExecutor] Reason: ${interactiveError}`);
    }
    return {
      success: false,
      error: interactiveError,
      stdout: '',
      stderr: interactiveError,
      exitCode: 1,
      command,
      workingDirectory: cwd,
      duration: 0,
      interactive: true
    };
  }

  if (debug) {
    console.log(`[BashExecutor] Executing command: "${command}"`);
    console.log(`[BashExecutor] Working directory: "${cwd}"`);
    console.log(`[BashExecutor] Timeout: ${timeout}ms`);
  }

  return new Promise((resolve, reject) => {
    // Create environment with non-interactive safety defaults.
    // These prevent commands from opening editors or TTY prompts
    // when stdin is not available (which would cause hangs).
    const processEnv = {
      ...process.env,
      ...env
    };
    // Only set defaults if not already provided by user config
    if (!processEnv.GIT_EDITOR) processEnv.GIT_EDITOR = 'true';
    if (!processEnv.GIT_TERMINAL_PROMPT) processEnv.GIT_TERMINAL_PROMPT = '0';

    // Check if this is a complex command (contains pipes, operators, etc.)
    const isComplex = isComplexCommand(command);

    let cmd, cmdArgs, useShell;

    if (isComplex) {
      // For complex commands, use sh -c to execute through shell
      // This is only reached if the permission checker allowed the complex command
      cmd = 'sh';
      cmdArgs = ['-c', command];
      useShell = false; // We explicitly use sh -c, not spawn's shell option
      if (debug) {
        console.log(`[BashExecutor] Complex command - using sh -c`);
      }
    } else {
      // Parse simple command for direct execution
      const args = parseCommandForExecution(command);
      if (!args || args.length === 0) {
        resolve({
          success: false,
          error: 'Failed to parse command',
          stdout: '',
          stderr: '',
          exitCode: 1,
          command,
          workingDirectory: cwd,
          duration: Date.now() - startTime
        });
        return;
      }
      [cmd, ...cmdArgs] = args;
      useShell = false;
    }

    // Spawn the process in a new session (detached: true → setsid on Linux).
    // This detaches the child from the parent's controlling terminal, making
    // /dev/tty unavailable. Any program that tries to open an interactive
    // editor or TTY prompt (e.g. vim from git rebase) will get ENXIO and
    // fail immediately instead of hanging forever.
    const child = spawn(cmd, cmdArgs, {
      cwd,
      env: processEnv,
      stdio: ['ignore', 'pipe', 'pipe'], // stdin ignored, capture stdout/stderr
      shell: useShell, // false for security
      detached: true, // new session — no controlling terminal
      windowsHide: true
    });

    let stdout = '';
    let stderr = '';
    let killed = false;
    let timeoutHandle;

    // Helper: kill the entire process group (negative PID) so that
    // sub-processes spawned by the command (e.g. an editor) are also killed.
    // Falls back to killing just the child if process.kill fails.
    const killProcessGroup = (signal) => {
      try {
        if (child.pid) process.kill(-child.pid, signal);
      } catch {
        try { child.kill(signal); } catch { /* already dead */ }
      }
    };

    // Set timeout
    if (timeout > 0) {
      timeoutHandle = setTimeout(() => {
        if (!killed) {
          killed = true;
          killProcessGroup('SIGTERM');

          // Force kill after 5 seconds if still running
          setTimeout(() => {
            if (child.exitCode === null) {
              killProcessGroup('SIGKILL');
            }
          }, 5000);
        }
      }, timeout);
    }

    // Handle stdout
    child.stdout.on('data', (data) => {
      const chunk = data.toString();
      if (stdout.length + chunk.length <= maxBuffer) {
        stdout += chunk;
      } else {
        // Buffer overflow
        if (!killed) {
          killed = true;
          killProcessGroup('SIGTERM');
        }
      }
    });

    // Handle stderr
    child.stderr.on('data', (data) => {
      const chunk = data.toString();
      if (stderr.length + chunk.length <= maxBuffer) {
        stderr += chunk;
      } else {
        // Buffer overflow
        if (!killed) {
          killed = true;
          killProcessGroup('SIGTERM');
        }
      }
    });

    // Handle process exit
    child.on('close', (code, signal) => {
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }

      const duration = Date.now() - startTime;
      
      if (debug) {
        console.log(`[BashExecutor] Command completed - Code: ${code}, Signal: ${signal}, Duration: ${duration}ms`);
        console.log(`[BashExecutor] Stdout length: ${stdout.length}, Stderr length: ${stderr.length}`);
      }

      let success = true;
      let error = '';

      if (killed) {
        success = false;
        if (stdout.length + stderr.length > maxBuffer) {
          error = `Command output exceeded maximum buffer size (${maxBuffer} bytes)`;
        } else {
          error = `Command timed out after ${timeout}ms`;
        }
      } else if (code !== 0) {
        success = false;
        error = `Command exited with code ${code}`;
      }

      resolve({
        success,
        error,
        stdout: stdout.trim(),
        stderr: stderr.trim(),
        exitCode: code,
        signal,
        command,
        workingDirectory: cwd,
        duration,
        killed
      });
    });

    // Handle spawn errors
    child.on('error', (error) => {
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }

      if (debug) {
        console.log(`[BashExecutor] Spawn error:`, error);
      }

      resolve({
        success: false,
        error: `Failed to execute command: ${error.message}`,
        stdout: '',
        stderr: '',
        exitCode: 1,
        command,
        workingDirectory: cwd,
        duration: Date.now() - startTime
      });
    });
  });
}


/**
 * Format execution result for display
 * @param {Object} result - Execution result
 * @param {boolean} [includeMetadata=false] - Include metadata in output
 * @returns {string} Formatted result
 */
export function formatExecutionResult(result, includeMetadata = false) {
  if (!result) {
    return 'No result available';
  }

  let output = '';

  // Add command info if metadata requested
  if (includeMetadata) {
    output += `Command: ${result.command}\n`;
    output += `Working directory: ${result.workingDirectory}\n`;
    output += `Duration: ${result.duration}ms\n`;
    output += `Exit Code: ${result.exitCode}\n`;
    if (result.signal) {
      output += `Signal: ${result.signal}\n`;
    }
    output += '\n';
  }

  // Add stdout if present
  if (result.stdout) {
    if (includeMetadata) {
      output += '--- STDOUT ---\n';
    }
    output += result.stdout;
    if (includeMetadata && result.stderr) {
      output += '\n';
    }
  }

  // Add stderr if present
  if (result.stderr) {
    if (includeMetadata) {
      if (result.stdout) output += '\n';
      output += '--- STDERR ---\n';
    } else if (result.stdout) {
      output += '\n--- STDERR ---\n';
    }
    output += result.stderr;
  }

  // Add error message if failed and no stderr
  if (!result.success && result.error && !result.stderr) {
    if (output) output += '\n';
    output += `Error: ${result.error}`;
  }

  // Add exit code for failed commands
  if (!result.success && result.exitCode !== undefined && result.exitCode !== 0) {
    if (output) output += '\n';
    output += `Exit code: ${result.exitCode}`;
  }

  return output || (result.success ? 'Command completed successfully (no output)' : 'Command failed (no output)');
}

/**
 * Validate execution options
 * @param {Object} options - Options to validate
 * @returns {Object} Validation result
 */
export function validateExecutionOptions(options = {}) {
  const errors = [];
  const warnings = [];

  // Check timeout
  if (options.timeout !== undefined) {
    if (typeof options.timeout !== 'number' || options.timeout < 0) {
      errors.push('timeout must be a non-negative number');
    } else if (options.timeout > 600000) { // 10 minutes
      warnings.push('timeout is very high (>10 minutes)');
    }
  }

  // Check maxBuffer
  if (options.maxBuffer !== undefined) {
    if (typeof options.maxBuffer !== 'number' || options.maxBuffer < 1024) {
      errors.push('maxBuffer must be at least 1024 bytes');
    } else if (options.maxBuffer > 100 * 1024 * 1024) { // 100MB
      warnings.push('maxBuffer is very high (>100MB)');
    }
  }

  // Check working directory
  if (options.workingDirectory) {
    if (typeof options.workingDirectory !== 'string') {
      errors.push('workingDirectory must be a string');
    } else if (!existsSync(options.workingDirectory)) {
      errors.push(`workingDirectory does not exist: ${options.workingDirectory}`);
    }
  }

  // Check environment
  if (options.env && typeof options.env !== 'object') {
    errors.push('env must be an object');
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings
  };
}