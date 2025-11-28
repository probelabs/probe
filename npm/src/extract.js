/**
 * Extract functionality for the probe package
 * @module extract
 */

import { exec, spawn } from 'child_process';
import { promisify } from 'util';
import { getBinaryPath, buildCliArgs, escapeString } from './utils.js';

const execAsync = promisify(exec);

/**
 * Flag mapping for extract options
 * Maps option keys to command-line flags
 */
const EXTRACT_FLAG_MAP = {
	allowTests: '--allow-tests',
	contextLines: '--context',
	format: '--format',
	inputFile: '--input-file'
};

/**
 * Extract code blocks from files
 *
 * @param {Object} options - Extract options
 * @param {string[]} [options.files] - Files to extract from (can include line numbers with colon, e.g., "/path/to/file.rs:10")
 * @param {string} [options.inputFile] - Path to a file containing unstructured text to extract file paths from
 * @param {string|Buffer} [options.content] - Content to pipe to stdin (e.g., git diff output). Alternative to inputFile.
 * @param {string} [options.path] - Base path for resolving relative file paths (sets working directory for the command)
 * @param {boolean} [options.allowTests] - Include test files
 * @param {number} [options.contextLines] - Number of context lines to include
 * @param {string} [options.format] - Output format ('markdown', 'plain', 'json', 'xml', 'color', 'outline-xml', 'outline-diff')
 * @param {Object} [options.binaryOptions] - Options for getting the binary
 * @param {boolean} [options.binaryOptions.forceDownload] - Force download even if binary exists
 * @param {string} [options.binaryOptions.version] - Specific version to download
 * @param {boolean} [options.json] - Return results as parsed JSON instead of string
 * @returns {Promise<string|Object>} - Extracted code as string or parsed JSON
 * @throws {Error} If the extraction fails
 */
export async function extract(options) {
	if (!options) {
		throw new Error('Options object is required');
	}

	// Either files, inputFile, or content must be provided
	const hasFiles = options.files && Array.isArray(options.files) && options.files.length > 0;
	const hasInputFile = !!options.inputFile;
	const hasContent = options.content !== undefined && options.content !== null;

	if (!hasFiles && !hasInputFile && !hasContent) {
		throw new Error('Extract requires one of: "files" (array of file paths), "inputFile" (path to input file), or "content" (string/buffer for stdin)');
	}

	// Get the binary path
	const binaryPath = await getBinaryPath(options.binaryOptions || {});

	// Build CLI arguments from options (excluding content which goes via stdin)
	const filteredOptions = { ...options };
	delete filteredOptions.content;
	const cliArgs = buildCliArgs(filteredOptions, EXTRACT_FLAG_MAP);

	// If json option is true, override format to json
	if (options.json && !options.format) {
		cliArgs.push('--format', 'json');
	}

	// Add files as positional arguments if provided
	if (hasFiles) {
		for (const file of options.files) {
			cliArgs.push(escapeString(file));
		}
	}

	// Get the working directory (path option sets cwd for relative file resolution)
	const cwd = options.path || process.cwd();

	// Create a single log record with all extract parameters (only in debug mode)
	if (process.env.DEBUG === '1') {
		let logMessage = `\nExtract:`;
		if (options.files && options.files.length > 0) {
			logMessage += ` files="${options.files.join(', ')}"`;
		}
		if (options.inputFile) logMessage += ` inputFile="${options.inputFile}"`;
		if (options.content) logMessage += ` content=(${typeof options.content === 'string' ? options.content.length : options.content.byteLength} bytes)`;
		if (options.path) logMessage += ` path="${options.path}"`;
		if (options.allowTests) logMessage += " allowTests=true";
		if (options.contextLines) logMessage += ` contextLines=${options.contextLines}`;
		if (options.format) logMessage += ` format=${options.format}`;
		if (options.json) logMessage += " json=true";
		console.error(logMessage);
	}

	// If content is provided, use spawn with stdin piping
	if (hasContent) {
		return extractWithStdin(binaryPath, cliArgs, options.content, options, cwd);
	}

	// Otherwise use exec for simple command execution
	const command = `${binaryPath} extract ${cliArgs.join(' ')}`;

	try {
		const { stdout, stderr } = await execAsync(command, { cwd });

		if (stderr) {
			console.error(`stderr: ${stderr}`);
		}

		return processExtractOutput(stdout, options);
	} catch (error) {
		// Enhance error message with command details
		const errorMessage = `Error executing extract command: ${error.message}\nCommand: ${command}\nCwd: ${cwd}`;
		throw new Error(errorMessage);
	}
}

/**
 * Extract with content piped to stdin
 * @private
 */
function extractWithStdin(binaryPath, cliArgs, content, options, cwd) {
	return new Promise((resolve, reject) => {
		const childProcess = spawn(binaryPath, ['extract', ...cliArgs], {
			stdio: ['pipe', 'pipe', 'pipe'],
			cwd
		});

		let stdout = '';
		let stderr = '';

		// Collect stdout
		childProcess.stdout.on('data', (data) => {
			stdout += data.toString();
		});

		// Collect stderr
		childProcess.stderr.on('data', (data) => {
			stderr += data.toString();
		});

		// Handle process exit
		childProcess.on('close', (code) => {
			if (stderr && process.env.DEBUG === '1') {
				console.error(`stderr: ${stderr}`);
			}

			if (code !== 0) {
				reject(new Error(`Extract command failed with exit code ${code}: ${stderr}`));
				return;
			}

			try {
				const result = processExtractOutput(stdout, options);
				resolve(result);
			} catch (error) {
				reject(error);
			}
		});

		// Handle errors
		childProcess.on('error', (error) => {
			reject(new Error(`Failed to spawn extract process: ${error.message}`));
		});

		// Write content to stdin and close
		if (typeof content === 'string') {
			childProcess.stdin.write(content);
		} else {
			childProcess.stdin.write(content);
		}
		childProcess.stdin.end();
	});
}

/**
 * Process extract output and add token usage information
 * @private
 */
function processExtractOutput(stdout, options) {
	// Parse the output to extract token usage information
	let tokenUsage = {
		requestTokens: 0,
		responseTokens: 0,
		totalTokens: 0
	};

	// Calculate approximate request tokens
	if (options.files && Array.isArray(options.files)) {
		tokenUsage.requestTokens = options.files.join(' ').length / 4;
	} else if (options.inputFile) {
		tokenUsage.requestTokens = options.inputFile.length / 4;
	} else if (options.content) {
		const contentLength = typeof options.content === 'string'
			? options.content.length
			: options.content.byteLength;
		tokenUsage.requestTokens = contentLength / 4;
	}

	// Try to extract token information from the output
	if (stdout.includes('Total tokens returned:')) {
		const tokenMatch = stdout.match(/Total tokens returned: (\d+)/);
		if (tokenMatch && tokenMatch[1]) {
			tokenUsage.responseTokens = parseInt(tokenMatch[1], 10);
			tokenUsage.totalTokens = tokenUsage.requestTokens + tokenUsage.responseTokens;
		}
	}

	// Add token usage information to the output
	let output = stdout;

	// Add token usage information at the end if not already present
	if (!output.includes('Token Usage:')) {
		output += `\nToken Usage:\n  Request tokens: ${tokenUsage.requestTokens}\n  Response tokens: ${tokenUsage.responseTokens}\n  Total tokens: ${tokenUsage.totalTokens}\n`;
	}

	// Parse JSON if requested or if format is json
	if (options.json || options.format === 'json') {
		try {
			const jsonOutput = JSON.parse(stdout);

			// Add token usage to JSON output
			if (!jsonOutput.token_usage) {
				jsonOutput.token_usage = {
					request_tokens: tokenUsage.requestTokens,
					response_tokens: tokenUsage.responseTokens,
					total_tokens: tokenUsage.totalTokens
				};
			}

			return jsonOutput;
		} catch (error) {
			console.error('Error parsing JSON output:', error);
			return output; // Fall back to string output with token usage
		}
	}

	return output;
}
