// Import tool generators from @buger/probe package
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE, listFilesByLevel } from '@buger/probe';
import { exec, spawn } from 'child_process';
import { promisify } from 'util';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';
import fs from 'fs';
import { promises as fsPromises } from 'fs';
import path from 'path';
import os from 'os';
import { glob } from 'glob';

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
				console.log(`[DEBUG] probeTool: Executing ${toolName} for session ${toolSessionId}`);
				console.log(`[DEBUG] probeTool: Received params:`, params);
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
	parameters: pluggableImplementTool.parameters,
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

// Create the listFiles tool
const baseListFilesTool = {
	name: "listFiles",
	description: 'List files in a specified directory',
	parameters: {
		type: 'object',
		properties: {
			directory: {
				type: 'string',
				description: 'The directory path to list files from. Defaults to current directory if not specified.'
			}
		},
		required: []
	},
	execute: async ({ directory = '.', sessionId }) => {
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

		const targetDir = path.resolve(currentWorkingDir, targetDirectory);

		// Validate that the target directory is within allowed folders
		if (allowedFolders.length > 0) {
			const isAllowed = allowedFolders.some(allowedFolder => {
				const resolvedAllowedFolder = path.resolve(currentWorkingDir, allowedFolder);
				return targetDir === resolvedAllowedFolder || targetDir.startsWith(resolvedAllowedFolder + path.sep);
			});

			if (!isAllowed) {
				const error = `Access denied: Directory '${targetDirectory}' is not within allowed folders: ${allowedFolders.join(', ')}`;
				if (debug) {
					console.log(`[DEBUG] ${error}`);
				}
				return {
					success: false,
					directory: targetDir,
					error: error,
					timestamp: new Date().toISOString()
				};
			}
		}

		if (debug) {
			console.log(`[DEBUG] Listing files in directory: ${targetDir}`);
		}

		try {
			// Read the directory contents
			const files = await fs.promises.readdir(targetDir, { withFileTypes: true });

			// Format the results
			const result = files.map(file => {
				const isDirectory = file.isDirectory();
				return {
					name: file.name,
					type: isDirectory ? 'directory' : 'file',
					path: path.join(targetDirectory, file.name)
				};
			});

			if (debug) {
				console.log(`[DEBUG] Found ${result.length} files/directories in ${targetDir}`);
			}

			return {
				success: true,
				directory: targetDir,
				files: result,
				timestamp: new Date().toISOString()
			};
		} catch (error) {
			console.error(`Error listing files in ${targetDir}:`, error);
			return {
				success: false,
				directory: targetDir,
				error: error.message || 'Unknown error listing files',
				timestamp: new Date().toISOString()
			};
		}
	}
};

// Create the searchFiles tool
const baseSearchFilesTool = {
	name: "searchFiles",
	description: 'Search for files using a glob pattern, recursively by default',
	parameters: {
		type: 'object',
		properties: {
			pattern: {
				type: 'string',
				description: 'The glob pattern to search for (e.g., "**/*.js", "*.md")'
			},
			directory: {
				type: 'string',
				description: 'The directory to search in. Defaults to current directory if not specified.'
			},
			recursive: {
				type: 'boolean',
				description: 'Whether to search recursively. Defaults to true.'
			}
		},
		required: ['pattern']
	},
	execute: async ({ pattern, directory, recursive = true, sessionId }) => {
		// Ensure directory defaults to current directory
		directory = directory || '.';

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

		const targetDir = path.resolve(currentWorkingDir, targetDirectory);

		// Validate that the target directory is within allowed folders
		if (allowedFolders.length > 0) {
			const isAllowed = allowedFolders.some(allowedFolder => {
				const resolvedAllowedFolder = path.resolve(currentWorkingDir, allowedFolder);
				return targetDir === resolvedAllowedFolder || targetDir.startsWith(resolvedAllowedFolder + path.sep);
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
		console.error(`Resolved target directory: ${targetDir}`);
		console.error(`Current working directory: ${currentWorkingDir}`);

		if (debug) {
			console.log(`[DEBUG] Searching for files with pattern: ${pattern}`);
			console.log(`[DEBUG] In directory: ${targetDir}`);
			console.log(`[DEBUG] Recursive: ${recursive}`);
		}

		// Validate pattern to prevent overly complex patterns
		if (pattern.includes('**/**') || pattern.split('*').length > 10) {
			console.error(`Pattern too complex: ${pattern}`);
			return {
				success: false,
				directory: targetDir,
				pattern: pattern,
				error: 'Pattern too complex. Please use a simpler glob pattern.',
				timestamp: new Date().toISOString()
			};
		}

		try {
			// Set glob options with timeout and limits
			const options = {
				cwd: targetDir,
				dot: true, // Include dotfiles
				nodir: true, // Only return files, not directories
				absolute: false, // Return paths relative to the search directory
				timeout: 10000, // 10 second timeout
				maxDepth: recursive ? 10 : 1, // Limit recursion depth
			};

			// If not recursive, modify the pattern to only search the top level
			const searchPattern = recursive ? pattern : pattern.replace(/^\*\*\//, '');

			console.error(`Starting glob search with pattern: ${searchPattern} in ${targetDir}`);
			console.error(`Glob options: ${JSON.stringify(options)}`);

			// Use a safer approach with manual file searching if the pattern is simple enough
			let files = [];

			// For simple patterns like "*.js" or "bin/*.js", use a more direct approach
			if (pattern.includes('*') && !pattern.includes('**') && pattern.split('/').length <= 2) {
				console.error(`Using direct file search for simple pattern: ${pattern}`);

				try {
					// Handle patterns like "dir/*.ext" or "*.ext"
					const parts = pattern.split('/');
					let searchDir = targetDir;
					let filePattern;

					if (parts.length === 2) {
						// Pattern like "dir/*.ext"
						searchDir = path.join(targetDir, parts[0]);
						filePattern = parts[1];
					} else {
						// Pattern like "*.ext"
						filePattern = parts[0];
					}

					console.error(`Searching in directory: ${searchDir} for files matching: ${filePattern}`);

					// Check if directory exists
					try {
						await fsPromises.access(searchDir);
					} catch (err) {
						console.error(`Directory does not exist: ${searchDir}`);
						return {
							success: true,
							directory: targetDir,
							pattern: pattern,
							recursive: recursive,
							files: [],
							count: 0,
							timestamp: new Date().toISOString()
						};
					}

					// Read directory contents
					const dirEntries = await fsPromises.readdir(searchDir, { withFileTypes: true });

					// Convert glob pattern to regex
					const regexPattern = filePattern
						.replace(/\./g, '\\.')
						.replace(/\*/g, '.*');
					const regex = new RegExp(`^${regexPattern}$`);

					// Filter files based on pattern
					files = dirEntries
						.filter(entry => entry.isFile() && regex.test(entry.name))
						.map(entry => {
							const relativePath = parts.length === 2
								? path.join(parts[0], entry.name)
								: entry.name;
							return relativePath;
						});

					console.error(`Direct search found ${files.length} files matching ${filePattern}`);
				} catch (err) {
					console.error(`Error in direct file search: ${err.message}`);
					// Fall back to glob if direct search fails
					console.error(`Falling back to glob search`);

					// Create a promise that rejects after a timeout
					const timeoutPromise = new Promise((_, reject) => {
						setTimeout(() => reject(new Error('Search operation timed out after 10 seconds')), 10000);
					});

					// Use glob without promisify since it might already return a Promise
					files = await Promise.race([
						glob(searchPattern, options),
						timeoutPromise
					]);
				}
			} else {
				console.error(`Using glob for complex pattern: ${pattern}`);

				// Create a promise that rejects after a timeout
				const timeoutPromise = new Promise((_, reject) => {
					setTimeout(() => reject(new Error('Search operation timed out after 10 seconds')), 10000);
				});

				// Use glob without promisify since it might already return a Promise
				files = await Promise.race([
					glob(searchPattern, options),
					timeoutPromise
				]);
			}

			console.error(`Search completed, found ${files.length} files in ${targetDir}`);
			console.error(`Pattern: ${pattern}, Recursive: ${recursive}`);

			if (debug) {
				console.log(`[DEBUG] Found ${files.length} files matching pattern ${pattern}`);
			}

			// Limit the number of results to prevent memory issues
			const maxResults = 1000;
			const limitedFiles = files.length > maxResults ? files.slice(0, maxResults) : files;

			if (files.length > maxResults) {
				console.warn(`Warning: Limited results to ${maxResults} files out of ${files.length} total matches`);
			}

			return {
				success: true,
				directory: targetDir,
				pattern: pattern,
				recursive: recursive,
				files: limitedFiles.map(file => path.join(targetDirectory, file)),
				count: limitedFiles.length,
				totalMatches: files.length,
				limited: files.length > maxResults,
				timestamp: new Date().toISOString()
			};
		} catch (error) {
			console.error(`Error searching files with pattern "${pattern}" in ${targetDir}:`, error);
			console.error(`Search parameters: directory="${targetDirectory}", recursive=${recursive}, sessionId=${sessionId}`);
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

// --- Backward Compatibility Layer (probeTool mapping to searchToolInstance) ---
// This might be less relevant if the AI is strictly using the new XML format,
// but keep it for potential direct API calls or older UI elements.
export const probeTool = {
	...searchToolInstance, // Inherit schema description etc. from the wrapped search tool
	name: "search", // Explicitly set name
	description: 'DEPRECATED: Use <search> tool instead. Search code using keywords.',
	// parameters: searchSchema, // Use the imported schema
	execute: async (params) => { // Expects { keywords, folder, ..., sessionId }
		const debug = process.env.DEBUG_CHAT === '1';
		if (debug) {
			console.log(`[DEBUG] probeTool (Compatibility Layer) executing for session ${params.sessionId}`);
		}

		// Map old params ('keywords', 'folder') to new ones ('query', 'path')
		const { keywords, folder, sessionId, ...rest } = params;
		const mappedParams = {
			query: keywords,
			path: folder || '.', // Default path if folder is missing
			sessionId: sessionId, // Pass session ID through
			...rest // Pass other params like allow_tests, maxResults etc.
		};

		if (debug) {
			console.log("[DEBUG] probeTool mapped params: ", mappedParams);
		}

		// Call the *wrapped* searchToolInstance execute function
		// It will handle cancellation checks and event emitting internally
		try {
			// Note: The name emitted by searchToolInstance will be 'search', not 'probeTool' or 'searchCode'
			const result = await searchToolInstance.execute(mappedParams);

			// Format the result for backward compatibility if needed by caller
			// The raw result from searchToolInstance is likely just the search results array/string
			const formattedResult = {
				results: result, // Assuming result is the direct data
				command: `probe search --query "${keywords}" --path "${folder || '.'}"`, // Reconstruct approx command
				timestamp: new Date().toISOString()
			};
			if (debug) {
				console.log("[DEBUG] probeTool compatibility layer returning formatted result.");
			}
			return formattedResult;

		} catch (error) {
			if (debug) {
				console.error(`[DEBUG] Error in probeTool compatibility layer:`, error);
			}
			// Error is already emitted by the wrapped searchToolInstance, just re-throw
			throw error;
		}
	}
};
// Export necessary items
export { DEFAULT_SYSTEM_MESSAGE, listFilesByLevel };
// Export the tool generator functions if needed elsewhere
export { searchTool, queryTool, extractTool };

// Export capabilities information for the new tools
export const toolCapabilities = {
	search: "Search code using keywords and patterns",
	query: "Query code with structured parameters for more precise results",
	extract: "Extract code blocks and context from files",
	implement: "Implement features or fix bugs using aider (requires --allow-edit)",
	listFiles: "List files and directories in a specified location",
	searchFiles: "Find files matching a glob pattern with recursive search capability"
};
