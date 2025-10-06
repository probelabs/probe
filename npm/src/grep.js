/**
 * Grep functionality for the probe package
 * @module grep
 */

import { exec } from 'child_process';
import { promisify } from 'util';
import { getBinaryPath, buildCliArgs } from './utils.js';

const execAsync = promisify(exec);

/**
 * Flag mapping for grep options
 * Maps option keys to command-line flags
 */
const GREP_FLAG_MAP = {
	ignoreCase: '-i',
	lineNumbers: '-n',
	count: '-c',
	filesWithMatches: '-l',
	filesWithoutMatches: '-L',
	invertMatch: '-v',
	beforeContext: '-B',
	afterContext: '-A',
	context: '-C',
	noGitignore: '--no-gitignore',
	color: '--color',
	maxCount: '-m'
};

/**
 * Standard grep-style search across files (works with any file type, not just code)
 *
 * This provides a cross-platform grep interface that works on any OS and file type.
 * Use this for searching non-code files (logs, config files, text files, etc.) that
 * are not supported by probe's semantic search.
 *
 * For code files, prefer using the `search()` function which provides semantic,
 * AST-aware search capabilities.
 *
 * @param {Object} options - Grep options
 * @param {string} options.pattern - Pattern to search for (regex)
 * @param {string|string[]} options.paths - Path(s) to search in
 * @param {boolean} [options.ignoreCase] - Case-insensitive search (-i)
 * @param {boolean} [options.lineNumbers] - Show line numbers (-n)
 * @param {boolean} [options.count] - Only show count of matches (-c)
 * @param {boolean} [options.filesWithMatches] - Only show filenames with matches (-l)
 * @param {boolean} [options.filesWithoutMatches] - Only show filenames without matches (-L)
 * @param {boolean} [options.invertMatch] - Invert match, show non-matching lines (-v)
 * @param {number} [options.beforeContext] - Lines of context before match (-B)
 * @param {number} [options.afterContext] - Lines of context after match (-A)
 * @param {number} [options.context] - Lines of context before and after match (-C)
 * @param {boolean} [options.noGitignore] - Don't respect .gitignore files (--no-gitignore)
 * @param {string} [options.color] - Colorize output: 'always', 'never', 'auto' (--color)
 * @param {number} [options.maxCount] - Stop after N matches per file (-m)
 * @param {Object} [options.binaryOptions] - Options for getting the binary
 * @param {boolean} [options.binaryOptions.forceDownload] - Force download even if binary exists
 * @param {string} [options.binaryOptions.version] - Specific version to download
 * @returns {Promise<string>} - Grep results as string
 * @throws {Error} If the grep operation fails
 *
 * @example
 * // Search for "error" in log files (case-insensitive)
 * const results = await grep({
 *   pattern: 'error',
 *   paths: '/var/log',
 *   ignoreCase: true,
 *   lineNumbers: true
 * });
 *
 * @example
 * // Count occurrences of "TODO" in project
 * const count = await grep({
 *   pattern: 'TODO',
 *   paths: '.',
 *   count: true
 * });
 *
 * @example
 * // Find files containing "config" with context
 * const matches = await grep({
 *   pattern: 'config',
 *   paths: '/etc',
 *   context: 2,
 *   filesWithMatches: true
 * });
 */
export async function grep(options) {
	if (!options || !options.pattern) {
		throw new Error('Pattern is required');
	}

	if (!options.paths) {
		throw new Error('Path(s) are required');
	}

	// Get the binary path
	const binaryPath = await getBinaryPath(options.binaryOptions || {});

	// Build CLI arguments for grep subcommand
	const cliArgs = ['grep'];

	// Add flags from GREP_FLAG_MAP
	for (const [key, flag] of Object.entries(GREP_FLAG_MAP)) {
		const value = options[key];
		if (value === undefined || value === null) continue;

		if (typeof value === 'boolean' && value) {
			// Boolean flag
			cliArgs.push(flag);
		} else if (typeof value === 'number') {
			// Numeric option
			cliArgs.push(flag, String(value));
		} else if (typeof value === 'string') {
			// String option (like color)
			cliArgs.push(flag, value);
		}
	}

	// Add pattern
	cliArgs.push(options.pattern);

	// Add paths (can be single string or array)
	const paths = Array.isArray(options.paths) ? options.paths : [options.paths];
	cliArgs.push(...paths);

	// Build command
	const cmd = `"${binaryPath}" ${cliArgs.map(arg => {
		// Quote arguments that contain spaces or special characters
		if (arg.includes(' ') || arg.includes('*') || arg.includes('?')) {
			return `"${arg}"`;
		}
		return arg;
	}).join(' ')}`;

	try {
		const { stdout, stderr } = await execAsync(cmd, {
			maxBuffer: 10 * 1024 * 1024, // 10MB buffer
			env: {
				...process.env,
				// Disable colors in stderr for cleaner output
				NO_COLOR: '1'
			}
		});

		// Return stdout (grep results)
		return stdout;
	} catch (error) {
		// Grep exit code 1 means "no matches found", which is not an error
		if (error.code === 1 && !error.stderr) {
			return error.stdout || '';
		}

		// Other errors are real failures
		const errorMessage = error.stderr || error.message || 'Unknown error';
		throw new Error(`Grep failed: ${errorMessage}`);
	}
}
