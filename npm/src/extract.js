/**
 * Extract functionality for the probe package
 * @module extract
 */

import { exec } from 'child_process';
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
	format: '--format'
};

/**
 * Extract code blocks from files
 * 
 * @param {Object} options - Extract options
 * @param {string[]} options.files - Files to extract from (can include line numbers with colon, e.g., "/path/to/file.rs:10")
 * @param {boolean} [options.allowTests] - Include test files
 * @param {number} [options.contextLines] - Number of context lines to include
 * @param {string} [options.format] - Output format ('markdown', 'plain', 'json')
 * @param {Object} [options.binaryOptions] - Options for getting the binary
 * @param {boolean} [options.binaryOptions.forceDownload] - Force download even if binary exists
 * @param {string} [options.binaryOptions.version] - Specific version to download
 * @param {boolean} [options.json] - Return results as parsed JSON instead of string
 * @returns {Promise<string|Object>} - Extracted code as string or parsed JSON
 * @throws {Error} If the extraction fails
 */
export async function extract(options) {
	if (!options || !options.files || !Array.isArray(options.files) || options.files.length === 0) {
		throw new Error('Files array is required and must not be empty');
	}

	// Get the binary path
	const binaryPath = await getBinaryPath(options.binaryOptions || {});

	// Build CLI arguments from options
	const cliArgs = buildCliArgs(options, EXTRACT_FLAG_MAP);

	// If json option is true, override format to json
	if (options.json && !options.format) {
		cliArgs.push('--format', 'json');
	}

	// Add files as positional arguments
	for (const file of options.files) {
		cliArgs.push(escapeString(file));
	}

	// Create a single log record with all extract parameters
	let logMessage = `Extract: files=${options.files.length} [${options.files.join(', ')}]`;
	if (options.contextLines) logMessage += ` contextLines=${options.contextLines}`;
	if (options.format) logMessage += ` format=${options.format}`;
	if (options.allowTests) logMessage += " allowTests=true";
	console.log(logMessage);

	// Execute command
	const command = `${binaryPath} extract ${cliArgs.join(' ')}`;

	try {
		const { stdout, stderr } = await execAsync(command);

		if (stderr) {
			console.error(`stderr: ${stderr}`);
		}

		// Count extracted code blocks
		let blockCount = 0;

		// Try to count code blocks from stdout
		const lines = stdout.split('\n');
		for (const line of lines) {
			if (line.startsWith('```') && !line.includes('```language')) {
				blockCount++;
			}
		}

		// Log the number of extracted code blocks
		console.log(`Extract results: ${blockCount} code blocks extracted`);

		// Parse JSON if requested or if format is json
		if (options.json || options.format === 'json') {
			try {
				return JSON.parse(stdout);
			} catch (error) {
				console.error('Error parsing JSON output:', error);
				return stdout; // Fall back to string output
			}
		}

		return stdout;
	} catch (error) {
		// Enhance error message with command details
		const errorMessage = `Error executing extract command: ${error.message}\nCommand: ${command}`;
		throw new Error(errorMessage);
	}
}