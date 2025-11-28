/**
 * Search functionality for the probe package
 * @module search
 */

import { execFile } from 'child_process';
import { promisify } from 'util';
import { getBinaryPath, buildCliArgs } from './utils.js';

const execFileAsync = promisify(execFile);

/**
 * Flag mapping for search options
 * Maps option keys to command-line flags
 */
const SEARCH_FLAG_MAP = {
	filesOnly: '--files-only',
	ignore: '--ignore',
	excludeFilenames: '--exclude-filenames',
	reranker: '--reranker',
	frequencySearch: '--frequency',
	exact: '--exact',
	strictElasticSyntax: '--strict-elastic-syntax',
	maxResults: '--max-results',
	maxBytes: '--max-bytes',
	maxTokens: '--max-tokens',
	allowTests: '--allow-tests',
	noMerge: '--no-merge',
	mergeThreshold: '--merge-threshold',
	session: '--session',
	timeout: '--timeout',
	language: '--language',
	format: '--format'
};

/**
 * Search code in a specified directory
 *
 * @param {Object} options - Search options
 * @param {string} options.path - Path to search in
 * @param {string} [options.cwd] - Working directory for resolving relative paths (defaults to process.cwd())
 * @param {string|string[]} options.query - Search query or queries
 * @param {boolean} [options.filesOnly] - Only output file paths
 * @param {string[]} [options.ignore] - Patterns to ignore
 * @param {boolean} [options.excludeFilenames] - Exclude filenames from search
 * @param {string} [options.reranker] - Reranking method ('hybrid', 'hybrid2', 'bm25', 'tfidf')
 * @param {boolean} [options.frequencySearch] - Use frequency-based search
 * @param {boolean} [options.exact] - Perform exact search without tokenization (case-insensitive)
 * @param {boolean} [options.strictElasticSyntax] - Enforce strict ElasticSearch query syntax (require explicit AND/OR operators and quotes)
 * @param {number} [options.maxResults] - Maximum number of results
 * @param {number} [options.maxBytes] - Maximum bytes to return
 * @param {number} [options.maxTokens] - Maximum tokens to return
 * @param {boolean} [options.allowTests] - Include test files
 * @param {boolean} [options.noMerge] - Don't merge adjacent blocks
 * @param {number} [options.mergeThreshold] - Merge threshold
 * @param {string} [options.session] - Session ID for caching results
 * @param {number} [options.timeout] - Timeout in seconds (default: 30)
 * @param {string} [options.language] - Limit search to files of a specific programming language
 * @param {string} [options.format] - Output format ('json', 'outline-xml', etc.)
 * @param {Object} [options.binaryOptions] - Options for getting the binary
 * @param {boolean} [options.binaryOptions.forceDownload] - Force download even if binary exists
 * @param {string} [options.binaryOptions.version] - Specific version to download
 * @param {boolean} [options.json] - Return results as parsed JSON instead of string
 * @returns {Promise<string|Object>} - Search results as string or parsed JSON
 * @throws {Error} If the search fails
 */
export async function search(options) {
	if (!options || !options.path) {
		throw new Error('Path is required');
	}

	if (!options.query) {
		throw new Error('Query is required');
	}

	// Get the binary path
	const binaryPath = await getBinaryPath(options.binaryOptions || {});

	// Build CLI arguments from options
	const cliArgs = buildCliArgs(options, SEARCH_FLAG_MAP);

	// Add format if specified, with json option taking precedence for backwards compatibility
	if (options.json && !options.format) {
		cliArgs.push('--format', 'json');
	} else if (options.format) {
		// Format is already handled by buildCliArgs through SEARCH_FLAG_MAP
		// but we need to ensure json parsing for json format
		if (options.format === 'json') {
			options.json = true;
		}
	}

	// Set default maxTokens if not provided
	if (!options.maxTokens) {
		options.maxTokens = 10000;
		cliArgs.push('--max-tokens', '10000');
	}

	// Set default timeout if not provided
	if (!options.timeout) {
		options.timeout = 30;
		cliArgs.push('--timeout', '30');
	}

	// Ensure language is properly passed if provided
	if (options.language) {
		// Ensure language flag is in cliArgs
		if (!cliArgs.includes('--language')) {
			cliArgs.push('--language', options.language);
		}
	}

	// Ensure exact search is properly passed if enabled
	if (options.exact) {
		// Ensure exact flag is in cliArgs
		if (!cliArgs.includes('--exact')) {
			cliArgs.push('--exact');
		}
	}

	// Add session ID from environment variable if not provided in options
	if (!options.session && process.env.PROBE_SESSION_ID) {
		options.session = process.env.PROBE_SESSION_ID;
	}

	// Add query and path as positional arguments
	const queries = Array.isArray(options.query) ? options.query : [options.query];

	// Get the working directory (cwd option for resolving relative paths)
	const cwd = options.cwd || process.cwd();

	// Create a single log record with all search parameters (only in debug mode)
	if (process.env.DEBUG === '1') {
		let logMessage = `\nSearch: query="${queries[0]}" path="${options.path}"`;
		if (options.cwd) logMessage += ` cwd="${options.cwd}"`;
		if (options.maxResults) logMessage += ` maxResults=${options.maxResults}`;
		logMessage += ` maxTokens=${options.maxTokens}`;
		logMessage += ` timeout=${options.timeout}`;
		if (options.allowTests) logMessage += " allowTests=true";
		if (options.language) logMessage += ` language=${options.language}`;
		if (options.exact) logMessage += " exact=true";
		if (options.session) logMessage += ` session=${options.session}`;
		console.error(logMessage);
	}
	// Build argument array for secure execution (no shell injection)
	const args = ['search', ...cliArgs];

	// Add positional arguments (query and path)
	if (queries.length > 0) {
		args.push(queries[0]);
	}
	args.push(options.path);

	// Debug logs
	if (process.env.DEBUG === '1') {
		console.error(`Executing: ${binaryPath} ${args.join(' ')}`);
	}

	try {
		// Execute with execFile (no shell, prevents command injection)
		const { stdout, stderr } = await execFileAsync(binaryPath, args, {
			cwd,
			timeout: options.timeout * 1000, // Convert seconds to milliseconds
			maxBuffer: 50 * 1024 * 1024 // 50MB buffer for large outputs
		});

		// Log after executing
		// console.error(`Command executed successfully`);

		if (stderr && process.env.DEBUG) {
			console.error(`stderr: ${stderr}`);
		}

		// Count results, tokens, and bytes
		let resultCount = 0;
		let tokenCount = 0;
		let bytesCount = 0;

		// Try to count results from stdout
		const lines = stdout.split('\n');
		for (const line of lines) {
			if (line.startsWith('```') && !line.includes('```language')) {
				resultCount++;
			}
		}

		// Look for the specific "Total bytes returned: X" line in the output
		const totalBytesMatch = stdout.match(/Total bytes returned:\s*(\d+)/i);
		if (totalBytesMatch && totalBytesMatch[1]) {
			bytesCount = parseInt(totalBytesMatch[1], 10);
		}

		// Look for the specific "Total tokens returned: X" line in the output
		const totalTokensMatch = stdout.match(/Total tokens returned:\s*(\d+)/i);
		if (totalTokensMatch && totalTokensMatch[1]) {
			tokenCount = parseInt(totalTokensMatch[1], 10);
		} else {
			// Try other patterns if the specific format isn't found
			const tokenMatch = stdout.match(/Tokens:?\s*(\d+)/i) ||
				stdout.match(/(\d+)\s*tokens/i) ||
				stdout.match(/token count:?\s*(\d+)/i);

			if (tokenMatch && tokenMatch[1]) {
				tokenCount = parseInt(tokenMatch[1], 10);
			} else {
				// If we still can't find the token count, use the default maxTokens value
				// This is a fallback, but the command should be returning the actual count
				tokenCount = options.maxTokens;
			}
		}

		// Log the results count, token count, and bytes count (only in debug mode)
		if (process.env.DEBUG === '1') {
			let resultsMessage = `\nSearch results: ${resultCount} matches, ${tokenCount} tokens`;
			if (bytesCount > 0) {
				resultsMessage += `, ${bytesCount} bytes`;
			}
			console.error(resultsMessage);
		}

		// Parse JSON if requested
		if (options.json) {
			try {
				return JSON.parse(stdout);
			} catch (error) {
				console.error('Error parsing JSON output:', error);
				return stdout; // Fall back to string output
			}
		}

		return stdout;
	} catch (error) {
		// Check if the error is a timeout
		if (error.code === 'ETIMEDOUT' || error.killed) {
			const timeoutMessage = `Search operation timed out after ${options.timeout} seconds.\nBinary: ${binaryPath}\nArgs: ${args.join(' ')}\nCwd: ${cwd}`;
			console.error(timeoutMessage);
			throw new Error(timeoutMessage);
		}

		// Enhance error message with command details
		const errorMessage = `Error executing search command: ${error.message}\nBinary: ${binaryPath}\nArgs: ${args.join(' ')}\nCwd: ${cwd}`;
		throw new Error(errorMessage);
	}
}