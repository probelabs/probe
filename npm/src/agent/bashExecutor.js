/**
 * Bash command executor with security and timeout controls
 * @module agent/bashExecutor
 */

import { spawn } from 'child_process';
import { resolve, join } from 'path';
import { existsSync } from 'fs';
import { parseCommandForExecution } from './bashCommandUtils.js';

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

  if (debug) {
    console.log(`[BashExecutor] Executing command: "${command}"`);
    console.log(`[BashExecutor] Working directory: "${cwd}"`);
    console.log(`[BashExecutor] Timeout: ${timeout}ms`);
  }

  return new Promise((resolve, reject) => {
    // Create environment
    const processEnv = {
      ...process.env,
      ...env
    };

    // Parse command for shell execution
    // We use shell: false for security, so we need to parse manually
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

    const [cmd, ...cmdArgs] = args;

    // Spawn the process
    const child = spawn(cmd, cmdArgs, {
      cwd,
      env: processEnv,
      stdio: ['ignore', 'pipe', 'pipe'], // stdin ignored, capture stdout/stderr
      shell: false, // For security
      windowsHide: true
    });

    let stdout = '';
    let stderr = '';
    let killed = false;
    let timeoutHandle;

    // Set timeout
    if (timeout > 0) {
      timeoutHandle = setTimeout(() => {
        if (!killed) {
          killed = true;
          child.kill('SIGTERM');
          
          // Force kill after 5 seconds if still running
          setTimeout(() => {
            if (child.exitCode === null) {
              child.kill('SIGKILL');
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
          child.kill('SIGTERM');
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
          child.kill('SIGTERM');
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