// Simplified tool wrapper for probe agent (based on examples/chat/probeTool.js)
import { listFilesByLevel } from '../index.js';
import { exec } from 'child_process';
import { promisify } from 'util';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';
import fs from 'fs';
import { promises as fsPromises } from 'fs';
import path from 'path';
import { glob } from 'glob';

// Create an event emitter for tool calls (simplified for single-shot operations)
export const toolCallEmitter = new EventEmitter();

// Map to track active tool executions by session ID
const activeToolExecutions = new Map();

// Function to check if a session has been cancelled
export function isSessionCancelled(sessionId) {
  return activeToolExecutions.get(sessionId)?.cancelled || false;
}

// Function to cancel all tool executions for a session
export function cancelToolExecutions(sessionId) {
  if (process.env.DEBUG === '1') {
    console.log(`Cancelling tool executions for session: ${sessionId}`);
  }
  const sessionData = activeToolExecutions.get(sessionId);
  if (sessionData) {
    sessionData.cancelled = true;
    return true;
  }
  return false;
}

// Function to register a new tool execution
function registerToolExecution(sessionId) {
  if (!sessionId) return;

  if (!activeToolExecutions.has(sessionId)) {
    activeToolExecutions.set(sessionId, { cancelled: false });
  } else {
    // Reset cancelled flag if session already exists for a new execution
    activeToolExecutions.get(sessionId).cancelled = false;
  }
}

// Function to clear tool execution data for a session
export function clearToolExecutionData(sessionId) {
  if (!sessionId) return;

  if (activeToolExecutions.has(sessionId)) {
    activeToolExecutions.delete(sessionId);
    if (process.env.DEBUG === '1') {
      console.log(`Cleared tool execution data for session: ${sessionId}`);
    }
  }
}

// Wrap the tools to emit events and handle cancellation
const wrapToolWithEmitter = (tool, toolName, baseExecute) => {
  return {
    ...tool, // Spread schema, description etc.
    execute: async (params) => { // The execute function now receives parsed params
      const debug = process.env.DEBUG === '1';
      // Get the session ID from params (passed down from ProbeAgent)
      const toolSessionId = params.sessionId || randomUUID();

      if (debug) {
        console.log(`[DEBUG] probeTool: Executing ${toolName} for session ${toolSessionId}`);
      }

      registerToolExecution(toolSessionId);

      let executionError = null;
      let result = null;

      try {
        // Emit the tool call start event
        const toolCallStartData = {
          timestamp: new Date().toISOString(),
          name: toolName,
          args: params,
          status: 'started'
        };

        if (debug) {
          console.log(`[DEBUG] probeTool: Emitting toolCallStart:${toolSessionId}`);
        }
        toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallStartData);

        // Check for cancellation before execution
        if (isSessionCancelled(toolSessionId)) {
          if (debug) {
            console.log(`Tool execution cancelled before start for ${toolSessionId}`);
          }
          throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
        }

        // Execute the base function
        result = await baseExecute(params);

        // Check for cancellation after execution
        if (isSessionCancelled(toolSessionId)) {
          if (debug) {
            console.log(`Tool execution cancelled after completion for ${toolSessionId}`);
          }
          throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
        }

      } catch (error) {
        executionError = error;
        if (debug) {
          console.error(`[DEBUG] probeTool: Error in ${toolName}:`, error);
        }
      }

      // Handle execution results and emit appropriate events
      if (executionError) {
        const toolCallErrorData = {
          timestamp: new Date().toISOString(),
          name: toolName,
          args: params,
          error: executionError.message || 'Unknown error',
          status: 'error'
        };
        if (debug) {
          console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (error)`);
        }
        toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallErrorData);

        throw executionError;
      } else {
        // If loop exited due to cancellation within the loop
        if (isSessionCancelled(toolSessionId)) {
          if (process.env.DEBUG === '1') {
            console.log(`Tool execution finished but session was cancelled for ${toolSessionId}`);
          }
          throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
        }

        // Emit the tool call completion event
        const toolCallData = {
          timestamp: new Date().toISOString(),
          name: toolName,
          args: params,
          // Safely preview result
          resultPreview: typeof result === 'string'
            ? (result.length > 200 ? result.substring(0, 200) + '...' : result)
            : (result ? JSON.stringify(result).substring(0, 200) + '...' : 'No Result'),
          status: 'completed'
        };
        if (debug) {
          console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (completed)`);
        }
        toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallData);

        return result;
      }
    }
  };
};

// Create wrapped tool instances - these will be created by the ProbeAgent
export function createWrappedTools(baseTools) {
  const wrappedTools = {};

  // Wrap search tool
  if (baseTools.searchTool) {
    wrappedTools.searchToolInstance = wrapToolWithEmitter(
      baseTools.searchTool, 
      'search', 
      baseTools.searchTool.execute
    );
  }

  // Wrap query tool
  if (baseTools.queryTool) {
    wrappedTools.queryToolInstance = wrapToolWithEmitter(
      baseTools.queryTool, 
      'query', 
      baseTools.queryTool.execute
    );
  }

  // Wrap extract tool
  if (baseTools.extractTool) {
    wrappedTools.extractToolInstance = wrapToolWithEmitter(
      baseTools.extractTool, 
      'extract', 
      baseTools.extractTool.execute
    );
  }

  // Wrap delegate tool
  if (baseTools.delegateTool) {
    wrappedTools.delegateToolInstance = wrapToolWithEmitter(
      baseTools.delegateTool, 
      'delegate', 
      baseTools.delegateTool.execute
    );
  }

  // Wrap bash tool
  if (baseTools.bashTool) {
    wrappedTools.bashToolInstance = wrapToolWithEmitter(
      baseTools.bashTool, 
      'bash', 
      baseTools.bashTool.execute
    );
  }

  return wrappedTools;
}

// Simple file listing tool with ls-like formatted output
export const listFilesTool = {
  execute: async (params) => {
    const { directory = '.', workingDirectory } = params;

    // Use the provided working directory, or fall back to process.cwd()
    const baseCwd = workingDirectory || process.cwd();

    // Security: Validate path to prevent traversal attacks
    const secureBaseDir = path.resolve(baseCwd);
    const targetDir = path.resolve(secureBaseDir, directory);
    if (!targetDir.startsWith(secureBaseDir + path.sep) && targetDir !== secureBaseDir) {
      throw new Error('Path traversal attempt detected. Access denied.');
    }

    const debug = process.env.DEBUG === '1';

    if (debug) {
      console.log(`[DEBUG] Listing files in directory: ${targetDir}`);
    }

    try {
      // Read the directory contents
      const files = await fsPromises.readdir(targetDir, { withFileTypes: true });

      // Format size for human readability
      const formatSize = (size) => {
        if (size < 1024) return `${size}B`;
        if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)}K`;
        if (size < 1024 * 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)}M`;
        return `${(size / (1024 * 1024 * 1024)).toFixed(1)}G`;
      };

      // Format the results as ls-style output
      const entries = await Promise.all(files.map(async (file) => {
        const isDirectory = file.isDirectory();
        const fullPath = path.join(targetDir, file.name);

        let size = 0;
        try {
          const stats = await fsPromises.stat(fullPath);
          size = stats.size;
        } catch (statError) {
          if (debug) {
            console.log(`[DEBUG] Could not stat file ${file.name}:`, statError.message);
          }
        }

        return {
          name: file.name,
          isDirectory,
          size
        };
      }));

      // Sort: directories first, then files, both alphabetically
      entries.sort((a, b) => {
        if (a.isDirectory && !b.isDirectory) return -1;
        if (!a.isDirectory && b.isDirectory) return 1;
        return a.name.localeCompare(b.name);
      });

      // Format entries
      const formatted = entries.map(entry => {
        const type = entry.isDirectory ? 'dir ' : 'file';
        const sizeStr = formatSize(entry.size).padStart(8);
        return `${type} ${sizeStr}  ${entry.name}`;
      });

      if (debug) {
        console.log(`[DEBUG] Found ${entries.length} files/directories in ${targetDir}`);
      }

      // Return as formatted text output
      const header = `${targetDir}:\n`;
      const output = header + formatted.join('\n');

      return output;
    } catch (error) {
      throw new Error(`Failed to list files: ${error.message}`);
    }
  }
};

// Simple file search tool with timeout protection
export const searchFilesTool = {
  execute: async (params) => {
    const { pattern, directory = '.', recursive = true, workingDirectory } = params;

    if (!pattern) {
      throw new Error('Pattern is required for file search');
    }

    // Security: Validate path to prevent traversal attacks
    const baseCwd = workingDirectory || process.cwd();
    const secureBaseDir = path.resolve(baseCwd);
    const targetDir = path.resolve(secureBaseDir, directory);
    if (!targetDir.startsWith(secureBaseDir + path.sep) && targetDir !== secureBaseDir) {
      throw new Error('Path traversal attempt detected. Access denied.');
    }

    // Validate pattern complexity to prevent DoS
    if (pattern.includes('**/**') || pattern.split('*').length > 10) {
      throw new Error('Pattern too complex. Please use a simpler glob pattern.');
    }

    try {
      const options = {
        cwd: targetDir,
        ignore: ['node_modules/**', '.git/**'],
        absolute: false
      };

      if (!recursive) {
        options.deep = 1;
      }

      // Create a timeout promise (10 seconds)
      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('Search operation timed out after 10 seconds')), 10000);
      });

      // Race glob against timeout
      const files = await Promise.race([
        glob(pattern, options),
        timeoutPromise
      ]);

      // Limit results to prevent memory issues
      const maxResults = 1000;
      if (files.length > maxResults) {
        return files.slice(0, maxResults);
      }

      return files;
    } catch (error) {
      throw new Error(`Failed to search files: ${error.message}`);
    }
  }
};

// Wrap the additional tools
export const listFilesToolInstance = wrapToolWithEmitter(listFilesTool, 'listFiles', listFilesTool.execute);
export const searchFilesToolInstance = wrapToolWithEmitter(searchFilesTool, 'searchFiles', searchFilesTool.execute);