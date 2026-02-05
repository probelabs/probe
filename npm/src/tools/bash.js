/**
 * Bash command execution tool for Vercel AI SDK
 * @module tools/bash
 */

import { tool } from 'ai';
import { resolve, isAbsolute, sep } from 'path';
import { BashPermissionChecker } from '../agent/bashPermissions.js';
import { executeBashCommand, formatExecutionResult, validateExecutionOptions } from '../agent/bashExecutor.js';
import { toRelativePath } from '../utils/path-validation.js';

/**
 * Bash tool generator
 * 
 * @param {Object} [options] - Configuration options
 * @param {Object} [options.bashConfig] - Bash-specific configuration
 * @param {string[]} [options.bashConfig.allow] - Additional allow patterns
 * @param {string[]} [options.bashConfig.deny] - Additional deny patterns
 * @param {boolean} [options.bashConfig.disableDefaultAllow] - Disable default allow list
 * @param {boolean} [options.bashConfig.disableDefaultDeny] - Disable default deny list
 * @param {number} [options.bashConfig.timeout=120000] - Command timeout in milliseconds
 * @param {string} [options.bashConfig.workingDirectory] - Default working directory
 * @param {Object} [options.bashConfig.env={}] - Default environment variables
 * @param {number} [options.bashConfig.maxBuffer] - Maximum output buffer size
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string} [options.cwd] - Working directory from probe config
 * @param {string[]} [options.allowedFolders] - Allowed directories for execution
 * @returns {Object} Configured bash tool
 */
export const bashTool = (options = {}) => {
  const {
    bashConfig = {},
    debug = false,
    cwd,
    allowedFolders = [],
    workspaceRoot = cwd || process.cwd(),
    tracer = null
  } = options;

  // Create permission checker with tracer for telemetry
  const permissionChecker = new BashPermissionChecker({
    allow: bashConfig.allow,
    deny: bashConfig.deny,
    disableDefaultAllow: bashConfig.disableDefaultAllow,
    disableDefaultDeny: bashConfig.disableDefaultDeny,
    debug,
    tracer
  });

  // Determine default working directory
  const getDefaultWorkingDirectory = () => {
    if (bashConfig.workingDirectory) {
      return bashConfig.workingDirectory;
    }
    if (cwd) {
      return cwd;
    }
    if (allowedFolders && allowedFolders.length > 0) {
      return allowedFolders[0];
    }
    return process.cwd();
  };

  return tool({
    name: 'bash',
    description: `Execute bash commands for system exploration and development tasks.

Security: This tool has built-in security with allow/deny lists. By default, only safe read-only commands are allowed for code exploration.

Parameters:
- command: (required) The bash command to execute
- workingDirectory: (optional) Directory to execute command in
- timeout: (optional) Command timeout in milliseconds
- env: (optional) Additional environment variables

Examples of allowed commands by default:
- File exploration: ls, cat, head, tail, find, grep
- Git operations: git status, git log, git diff, git show
- Package info: npm list, pip list, cargo --version
- System info: whoami, pwd, uname, date

Dangerous commands are blocked by default (rm -rf, sudo, npm install, etc.)`,

    inputSchema: {
      type: 'object',
      properties: {
        command: {
          type: 'string',
          description: 'The bash command to execute'
        },
        workingDirectory: {
          type: 'string',
          description: 'Directory to execute the command in (optional)'
        },
        timeout: {
          type: 'number',
          description: 'Command timeout in milliseconds (optional)',
          minimum: 1000,
          maximum: 600000
        },
        env: {
          type: 'object',
          description: 'Additional environment variables (optional)',
          additionalProperties: {
            type: 'string'
          }
        }
      },
      required: ['command'],
      additionalProperties: false
    },

    execute: async ({ command, workingDirectory, timeout, env }) => {
      try {
        // Validate command
        if (command === null || command === undefined || typeof command !== 'string') {
          return 'Error: Command is required and must be a string';
        }

        if (command.trim().length === 0) {
          return 'Error: Command cannot be empty';
        }

        // Check permissions
        const permissionResult = permissionChecker.check(command.trim());
        if (!permissionResult.allowed) {
          if (debug) {
            console.log(`[BashTool] Permission denied for command: "${command}"`);
            console.log(`[BashTool] Reason: ${permissionResult.reason}`);
          }
          return `Permission denied: ${permissionResult.reason}

This command is not allowed by the current security policy. 

Common reasons:
1. The command is in the deny list (potentially dangerous)
2. The command is not in the allow list (not a recognized safe command)

If you believe this command should be allowed, you can:
- Use the --bash-allow option to add specific patterns
- Use the --no-default-bash-deny flag to remove default restrictions (not recommended)

For code exploration, try these safe alternatives:
- ls, cat, head, tail for file operations
- find, grep, rg for searching
- git status, git log, git show for git operations
- npm list, pip list for package information`;
        }

        // Determine working directory
        const defaultDir = getDefaultWorkingDirectory();
        // Resolve relative paths against the default working directory context, not process.cwd()
        const workingDir = workingDirectory
          ? (isAbsolute(workingDirectory) ? resolve(workingDirectory) : resolve(defaultDir, workingDirectory))
          : defaultDir;

        // Validate working directory is within allowed folders if specified
        if (allowedFolders && allowedFolders.length > 0) {
          const resolvedWorkingDir = resolve(workingDir);
          const isAllowed = allowedFolders.some(folder => {
            const resolvedFolder = resolve(folder);
            // Use exact match OR startsWith with separator to prevent bypass attacks
            // e.g., '/tmp-malicious' should NOT match allowed folder '/tmp'
            return resolvedWorkingDir === resolvedFolder || resolvedWorkingDir.startsWith(resolvedFolder + sep);
          });

          if (!isAllowed) {
            const relativeDir = toRelativePath(workingDir, workspaceRoot);
            const relativeAllowed = allowedFolders.map(f => toRelativePath(f, workspaceRoot));
            return `Error: Working directory "${relativeDir}" is not within allowed folders: ${relativeAllowed.join(', ')}`;
          }
        }

        // Prepare execution options
        const executionOptions = {
          workingDirectory: workingDir,
          timeout: timeout || bashConfig.timeout || 120000,
          env: { ...bashConfig.env, ...env },
          maxBuffer: bashConfig.maxBuffer,
          debug
        };

        // Validate execution options
        const validation = validateExecutionOptions(executionOptions);
        if (!validation.valid) {
          return `Error: Invalid execution options: ${validation.errors.join(', ')}`;
        }

        if (validation.warnings.length > 0 && debug) {
          console.log('[BashTool] Warnings:', validation.warnings);
        }

        if (debug) {
          console.log(`[BashTool] Executing command: "${command}"`);
          console.log(`[BashTool] Working directory: "${workingDir}"`);
          console.log(`[BashTool] Timeout: ${executionOptions.timeout}ms`);
        }

        // Execute command
        const result = await executeBashCommand(command.trim(), executionOptions);

        if (debug) {
          console.log(`[BashTool] Command completed - Success: ${result.success}, Duration: ${result.duration}ms`);
        }

        // Format and return result
        const formattedResult = formatExecutionResult(result, debug);
        
        // Add metadata for failed commands
        if (!result.success) {
          let errorInfo = `\n\nCommand failed with exit code ${result.exitCode}`;
          if (result.killed) {
            errorInfo += ` (${result.error})`;
          }
          return formattedResult + errorInfo;
        }

        return formattedResult;

      } catch (error) {
        if (debug) {
          console.error('[BashTool] Execution error:', error);
        }
        return `Error executing bash command: ${error.message}`;
      }
    }
  });
};