/**
 * Query functionality for the probe package
 * @module query
 */

import { exec } from 'child_process';
import { promisify } from 'util';
import { getBinaryPath, buildCliArgs, escapeString } from './utils.js';

const execAsync = promisify(exec);

/**
 * Flag mapping for query options
 * Maps option keys to command-line flags
 */
const QUERY_FLAG_MAP = {
	language: '--language',
	ignore: '--ignore',
	allowTests: '--allow-tests',
	maxResults: '--max-results',
	format: '--format'
};

/**
 * Query code in a specified directory using tree-sitter patterns
 * 
 * @param {Object} options - Query options
 * @param {string} options.path - Path to search in
 * @param {string} options.pattern - The ast-grep pattern to search for
 * @param {string} [options.language] - Programming language to search in
 * @param {string[]} [options.ignore] - Patterns to ignore
 * @param {boolean} [options.allowTests] - Include test files
 * @param {number} [options.maxResults] - Maximum number of results
 * @param {string} [options.format] - Output format ('markdown', 'plain', 'json', 'color')
 * @param {Object} [options.binaryOptions] - Options for getting the binary
 * @param {boolean} [options.binaryOptions.forceDownload] - Force download even if binary exists
 * @param {string} [options.binaryOptions.version] - Specific version to download
 * @param {boolean} [options.json] - Return results as parsed JSON instead of string
 * @returns {Promise<string|Object>} - Query results as string or parsed JSON
 * @throws {Error} If the query fails
 */
export async function query(options) {
	if (!options || !options.path) {
		throw new Error('Path is required');
	}

	if (!options.pattern) {
		throw new Error('Pattern is required');
	}

	// Get the binary path
	const binaryPath = await getBinaryPath(options.binaryOptions || {});

	// Build CLI arguments from options
	const cliArgs = buildCliArgs(options, QUERY_FLAG_MAP);

	// If json option is true, override format to json
	if (options.json && !options.format) {
		cliArgs.push('--format', 'json');
	}

	// Add pattern and path as positional arguments
	cliArgs.push(escapeString(options.pattern), escapeString(options.path));

	// Create a single log record with all query parameters (only in debug mode)
	if (process.env.DEBUG === '1') {
		let logMessage = `Query: pattern="${options.pattern}" path="${options.path}"`;
		if (options.language) logMessage += ` language=${options.language}`;
		if (options.maxResults) logMessage += ` maxResults=${options.maxResults}`;
		if (options.allowTests) logMessage += " allowTests=true";
		console.error(logMessage);
	}

	// Execute command
	const command = `${binaryPath} query ${cliArgs.join(' ')}`;

	try {
		const { stdout, stderr } = await execAsync(command);

		if (stderr) {
			console.error(`stderr: ${stderr}`);
		}

		// Count results
		let resultCount = 0;

		// Try to count results from stdout
		const lines = stdout.split('\n');
		for (const line of lines) {
			if (line.startsWith('```') && !line.includes('```language')) {
				resultCount++;
			}
		}

		// Log the results count (only in debug mode)
		if (process.env.DEBUG === '1') {
			console.error(`Query results: ${resultCount} matches`);
		}

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
		const errorMessage = `Error executing query command: ${error.message}\nCommand: ${command}`;
		throw new Error(errorMessage);
	}
}