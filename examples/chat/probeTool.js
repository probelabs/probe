// Import tool generators and instances from @probelabs/probe package
import {
	searchTool,
	queryTool,
	extractTool,
	DEFAULT_SYSTEM_MESSAGE,
	listFilesToolInstance as packageListFilesToolInstance,
	searchFilesToolInstance as packageSearchFilesToolInstance
} from '@probelabs/probe';
import { spawn } from 'child_process';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';

// Import the new pluggable implementation tool
import { createImplementTool } from './implement/core/ImplementTool.js';

// Create an event emitter for tool calls
export const toolCallEmitter = new EventEmitter();

// Map to track active tool executions by session ID
const activeToolExecutions = new Map();

// Function to check if a session has been cancelled
export function isSessionCancelled(sessionId) {
	return activeToolExecutions.get(sessionId)?.cancelled || false;
}

// Function to cancel all tool executions for a session
export function cancelToolExecutions(sessionId) {
	// Only log if not in non-interactive mode or if in debug mode
	if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
		console.log(`Cancelling tool executions for session: ${sessionId}`);
	}
	const sessionData = activeToolExecutions.get(sessionId);
	if (sessionData) {
		sessionData.cancelled = true;
		// Only log if not in non-interactive mode or if in debug mode
		if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
			console.log(`Session ${sessionId} marked as cancelled`);
		}
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
		// Only log if not in non-interactive mode or if in debug mode
		if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
			console.log(`Cleared tool execution data for session: ${sessionId}`);
		}
	}
}

// Generate a default session ID (less relevant now, session is managed per-chat)
const defaultSessionId = randomUUID();
// Only log session ID in debug mode
if (process.env.DEBUG_CHAT === '1') {
	console.log(`Generated default session ID (probeTool.js): ${defaultSessionId}`);
}

// Create configured tools with the session ID
// Note: These configOptions are less critical now as sessionId is passed explicitly
const configOptions = {
	sessionId: defaultSessionId,
	debug: process.env.DEBUG_CHAT === '1'
};

// Helper function to truncate long argument values for logging
function truncateArgValue(value, maxLength = 200) {
	if (typeof value !== 'string') {
		value = JSON.stringify(value);
	}
	if (value.length <= maxLength * 2) {
		return value;
	}
	// Show first 200 and last 200 characters
	return `${value.substring(0, maxLength)}...${value.substring(value.length - maxLength)}`;
}

// Helper function to format tool arguments for debug logging
function formatToolArgs(args) {
	const formatted = {};
	for (const [key, value] of Object.entries(args)) {
		formatted[key] = truncateArgValue(value);
	}
	return formatted;
}

// Create the base tools using the imported generators
const baseSearchTool = searchTool(configOptions);
const baseQueryTool = queryTool(configOptions);
const baseExtractTool = extractTool(configOptions);


// Wrap the tools to emit events and handle cancellation
const wrapToolWithEmitter = (tool, toolName, baseExecute) => {
	return {
		...tool, // Spread schema, description etc.
		execute: async (params) => { // The execute function now receives parsed params
			const debug = process.env.DEBUG_CHAT === '1';
			// Get the session ID from params (passed down from probeChat.js)
			const toolSessionId = params.sessionId || defaultSessionId; // Fallback, but should always have sessionId

			if (debug) {
				console.log(`\n[DEBUG] ========================================`);
				console.log(`[DEBUG] Tool Call: ${toolName}`);
				console.log(`[DEBUG] Session: ${toolSessionId}`);
				console.log(`[DEBUG] Arguments:`);
				const formattedArgs = formatToolArgs(params);
				for (const [key, value] of Object.entries(formattedArgs)) {
					console.log(`[DEBUG]   ${key}: ${value}`);
				}
				console.log(`[DEBUG] ========================================\n`);
			}

			// Register this tool execution (and reset cancel flag if needed)
			registerToolExecution(toolSessionId);

			// Check if this session has been cancelled *before* execution
			if (isSessionCancelled(toolSessionId)) {
				// Only log if not in non-interactive mode or if in debug mode
				console.error(`Tool execution cancelled BEFORE starting for session ${toolSessionId}`);
				throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
			}
			// Only log if not in non-interactive mode or if in debug mode
			console.error(`Executing ${toolName} for session ${toolSessionId}`); // Simplified log

			// Remove sessionId from params before passing to base tool if it expects only schema params
			const { sessionId, ...toolParams } = params;

			try {
				// Emit a tool call start event
				const toolCallStartData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: toolParams, // Log schema params
					status: 'started'
				};
				if (debug) {
					console.log(`[DEBUG] probeTool: Emitting toolCallStart:${toolSessionId}`);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallStartData);

				// Execute the original tool's execute function with schema params
				// Use a promise-based approach with cancellation check
				let result = null;
				let executionError = null;

				const executionPromise = baseExecute(toolParams).catch(err => {
					executionError = err; // Capture error
				});

				const checkInterval = 50; // Check every 50ms
				while (result === null && executionError === null) {
					if (isSessionCancelled(toolSessionId)) {
						console.error(`Tool execution cancelled DURING execution for session ${toolSessionId}`);
						// Attempt to signal cancellation if the underlying tool supports it (future enhancement)
						// For now, just throw the cancellation error
						throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
					}
					// Check if promise is resolved or rejected
					const status = await Promise.race([
						executionPromise.then(() => 'resolved').catch(() => 'rejected'),
						new Promise(resolve => setTimeout(() => resolve('pending'), checkInterval))
					]);

					if (status === 'resolved') {
						result = await executionPromise; // Get the result
					} else if (status === 'rejected') {
						// Error already captured by the catch block on executionPromise
						break;
					}
					// If 'pending', continue loop
				}

				// If loop exited due to error
				if (executionError) {
					throw executionError;
				}

				// If loop exited due to cancellation within the loop
				if (isSessionCancelled(toolSessionId)) {
					// Only log if not in non-interactive mode or if in debug mode
					if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
						console.log(`Tool execution finished but session was cancelled for ${toolSessionId}`);
					}
					throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
				}


				// Emit the tool call completion event
				const toolCallData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: toolParams,
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
			} catch (error) {
				// If it's a cancellation error, re-throw it directly
				if (error.message.includes('cancelled for session')) {
					// Only log if not in non-interactive mode or if in debug mode
					if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
						console.log(`Caught cancellation error for ${toolName} in session ${toolSessionId}`);
					}
					// Emit cancellation event? Or let the caller handle it? Let caller handle.
					throw error;
				}

				// Handle other execution errors
				if (debug) {
					console.error(`[DEBUG] probeTool: Error executing ${toolName}:`, error);
				}

				// Emit a tool call error event
				const toolCallErrorData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: toolParams,
					error: error.message || 'Unknown error',
					status: 'error'
				};
				if (debug) {
					console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (error)`);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallErrorData);

				throw error; // Re-throw the error to be caught by probeChat.js loop
			}
		}
	};
};

// Create the implement tool using the new pluggable system
const implementToolConfig = {
	enabled: process.env.ALLOW_EDIT === '1' || process.argv.includes('--allow-edit'),
	backendConfig: {
		// Configuration can be extended here
	}
};

const pluggableImplementTool = createImplementTool(implementToolConfig);

// Create a compatibility wrapper for the old interface
const baseImplementTool = {
	name: "implement",
	description: pluggableImplementTool.description,
	inputSchema: pluggableImplementTool.inputSchema,
	execute: async ({ task, autoCommits = false, prompt, sessionId }) => {
		const debug = process.env.DEBUG_CHAT === '1';
		
		if (debug) {
			console.log(`[DEBUG] Executing implementation with task: ${task}`);
			console.log(`[DEBUG] Auto-commits: ${autoCommits}`);
			console.log(`[DEBUG] Session ID: ${sessionId}`);
			if (prompt) console.log(`[DEBUG] Custom prompt: ${prompt}`);
		}

		// Check if the tool is enabled
		if (!implementToolConfig.enabled) {
			return {
				success: false,
				output: null,
				error: 'Implementation tool is not enabled. Use --allow-edit flag to enable.',
				command: null,
				timestamp: new Date().toISOString(),
				prompt: prompt || task
			};
		}

		try {
			// Use the new pluggable implementation tool
			const result = await pluggableImplementTool.execute({
				task: prompt || task, // Use prompt if provided, otherwise use task
				autoCommit: autoCommits,
				sessionId: sessionId,
				// Pass through any additional options that might be useful
				context: {
					workingDirectory: process.cwd()
				}
			});

			// The result is already in the expected format
			return result;

		} catch (error) {
			// Handle any unexpected errors
			console.error(`Error in implement tool:`, error);
			return {
				success: false,
				output: null,
				error: error.message || 'Unknown error in implementation tool',
				command: null,
				timestamp: new Date().toISOString(),
				prompt: prompt || task
			};
		}
	}
};

// Wrapper for listFiles tool with ALLOWED_FOLDERS security
const baseListFilesTool = {
	...packageListFilesToolInstance,
	execute: async (params) => {
		const { directory = '.', sessionId } = params;
		const debug = process.env.DEBUG_CHAT === '1';
		const currentWorkingDir = process.cwd();

		// Get allowed folders from environment variable
		const allowedFoldersEnv = process.env.ALLOWED_FOLDERS;
		let allowedFolders = [];

		if (allowedFoldersEnv) {
			allowedFolders = allowedFoldersEnv.split(',').map(folder => folder.trim()).filter(folder => folder.length > 0);
		}

		// Handle default directory behavior when ALLOWED_FOLDERS is set
		let targetDirectory = directory;
		if (allowedFolders.length > 0 && (directory === '.' || directory === './')) {
			// Use the first allowed folder if directory is current directory
			targetDirectory = allowedFolders[0];
			if (debug) {
				console.log(`[DEBUG] Redirecting from '${directory}' to first allowed folder: ${targetDirectory}`);
			}
		}

		const targetDir = require('path').resolve(currentWorkingDir, targetDirectory);

		// Validate that the target directory is within allowed folders
		if (allowedFolders.length > 0) {
			const isAllowed = allowedFolders.some(allowedFolder => {
				const resolvedAllowedFolder = require('path').resolve(currentWorkingDir, allowedFolder);
				return targetDir === resolvedAllowedFolder || targetDir.startsWith(resolvedAllowedFolder + require('path').sep);
			});

			if (!isAllowed) {
				const error = `Access denied: Directory '${targetDirectory}' is not within allowed folders: ${allowedFolders.join(', ')}`;
				if (debug) {
					console.log(`[DEBUG] ${error}`);
				}
				return `Error: ${error}`;
			}
		}

		// Call the package tool with workingDirectory parameter
		return packageListFilesToolInstance.execute({
			...params,
			directory: targetDirectory,
			workingDirectory: currentWorkingDir
		});
	}
};

// Wrapper for searchFiles tool with ALLOWED_FOLDERS security
const baseSearchFilesTool = {
	...packageSearchFilesToolInstance,
	execute: async (params) => {
		const { pattern, directory = '.', recursive = true, sessionId } = params;
		const debug = process.env.DEBUG_CHAT === '1';
		const currentWorkingDir = process.cwd();

		// Get allowed folders from environment variable
		const allowedFoldersEnv = process.env.ALLOWED_FOLDERS;
		let allowedFolders = [];

		if (allowedFoldersEnv) {
			allowedFolders = allowedFoldersEnv.split(',').map(folder => folder.trim()).filter(folder => folder.length > 0);
		}

		// Handle default directory behavior when ALLOWED_FOLDERS is set
		let targetDirectory = directory;
		if (allowedFolders.length > 0 && (directory === '.' || directory === './')) {
			// Use the first allowed folder if directory is current directory
			targetDirectory = allowedFolders[0];
			if (debug) {
				console.log(`[DEBUG] Redirecting from '${directory}' to first allowed folder: ${targetDirectory}`);
			}
		}

		const targetDir = require('path').resolve(currentWorkingDir, targetDirectory);

		// Validate that the target directory is within allowed folders
		if (allowedFolders.length > 0) {
			const isAllowed = allowedFolders.some(allowedFolder => {
				const resolvedAllowedFolder = require('path').resolve(currentWorkingDir, allowedFolder);
				return targetDir === resolvedAllowedFolder || targetDir.startsWith(resolvedAllowedFolder + require('path').sep);
			});

			if (!isAllowed) {
				const error = `Access denied: Directory '${targetDirectory}' is not within allowed folders: ${allowedFolders.join(', ')}`;
				if (debug) {
					console.log(`[DEBUG] ${error}`);
				}
				return {
					success: false,
					directory: targetDir,
					pattern: pattern,
					error: error,
					timestamp: new Date().toISOString()
				};
			}
		}

		// Log execution parameters to stderr for visibility
		console.error(`Executing searchFiles with params: pattern="${pattern}", directory="${targetDirectory}", recursive=${recursive}`);

		try {
			// Call the package tool with workingDirectory parameter
			const files = await packageSearchFilesToolInstance.execute({
				...params,
				directory: targetDirectory,
				recursive,
				workingDirectory: currentWorkingDir
			});

			if (debug) {
				console.log(`[DEBUG] Found ${files.length} files matching pattern ${pattern}`);
			}

			// Return in the expected format for backward compatibility
			return {
				success: true,
				directory: targetDir,
				pattern: pattern,
				recursive: recursive,
				files: files.map(file => require('path').join(targetDirectory, file)),
				count: files.length,
				totalMatches: files.length,
				limited: false,
				timestamp: new Date().toISOString()
			};
		} catch (error) {
			console.error(`Error searching files with pattern "${pattern}" in ${targetDir}:`, error);
			return {
				success: false,
				directory: targetDir,
				pattern: pattern,
				error: error.message || 'Unknown error searching files',
				timestamp: new Date().toISOString()
			};
		}
	}
};

// Export the wrapped tool instances
export const searchToolInstance = wrapToolWithEmitter(baseSearchTool, 'search', baseSearchTool.execute);
export const queryToolInstance = wrapToolWithEmitter(baseQueryTool, 'query', baseQueryTool.execute);
export const extractToolInstance = wrapToolWithEmitter(baseExtractTool, 'extract', baseExtractTool.execute);
export const implementToolInstance = wrapToolWithEmitter(baseImplementTool, 'implement', baseImplementTool.execute);
export const listFilesToolInstance = wrapToolWithEmitter(baseListFilesTool, 'listFiles', baseListFilesTool.execute);
export const searchFilesToolInstance = wrapToolWithEmitter(baseSearchFilesTool, 'searchFiles', baseSearchFilesTool.execute);

// Log available tools at startup in debug mode
if (process.env.DEBUG_CHAT === '1') {
	console.log('\n[DEBUG] ========================================');
	console.log('[DEBUG] Probe Tools Loaded:');
	console.log('[DEBUG]   - search: Search for code patterns');
	console.log('[DEBUG]   - query: Semantic code search');
	console.log('[DEBUG]   - extract: Extract code snippets');
	console.log('[DEBUG]   - implement: Generate code implementations');
	console.log('[DEBUG]   - listFiles: List directory contents');
	console.log('[DEBUG]   - searchFiles: Search files by pattern');
	console.log('[DEBUG] ========================================\n');
}

