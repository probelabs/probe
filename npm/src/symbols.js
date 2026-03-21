/**
 * Symbols functionality for the probe package
 * @module symbols
 */

import { spawn } from 'child_process';
import { getBinaryPath, escapeString } from './utils.js';
import { validateCwdPath } from './utils/path-validation.js';

/**
 * List symbols (functions, structs, classes, constants, etc.) in files
 *
 * @param {Object} options - Symbols options
 * @param {string[]} options.files - Files to list symbols from
 * @param {string} [options.cwd] - Working directory for resolving relative file paths
 * @param {boolean} [options.allowTests] - Include test functions/methods
 * @param {Object} [options.binaryOptions] - Options for getting the binary
 * @returns {Promise<Object[]>} - Array of FileSymbols objects (parsed JSON)
 * @throws {Error} If the command fails
 */
export async function symbols(options) {
	if (!options) {
		throw new Error('Options object is required');
	}

	if (!options.files || !Array.isArray(options.files) || options.files.length === 0) {
		throw new Error('At least one file path is required');
	}

	const binaryPath = await getBinaryPath(options.binaryOptions || {});
	const cwd = await validateCwdPath(options.cwd);

	const args = ['symbols', '--format', 'json'];

	if (options.allowTests) {
		args.push('--allow-tests');
	}

	for (const file of options.files) {
		args.push(escapeString(file));
	}

	if (process.env.DEBUG === '1') {
		console.error(`\nSymbols: files="${options.files.join(', ')}" cwd="${cwd}"`);
	}

	return new Promise((resolve, reject) => {
		const childProcess = spawn(binaryPath, args, {
			stdio: ['pipe', 'pipe', 'pipe'],
			cwd
		});

		let stdout = '';
		let stderr = '';

		childProcess.stdout.on('data', (data) => {
			stdout += data.toString();
		});

		childProcess.stderr.on('data', (data) => {
			stderr += data.toString();
		});

		childProcess.on('close', (code) => {
			if (stderr && process.env.DEBUG === '1') {
				console.error(`stderr: ${stderr}`);
			}

			if (code !== 0) {
				reject(new Error(`Symbols command failed with exit code ${code}: ${stderr}`));
				return;
			}

			try {
				const result = JSON.parse(stdout);
				resolve(result);
			} catch (error) {
				reject(new Error(`Failed to parse symbols output: ${error.message}`));
			}
		});

		childProcess.on('error', (error) => {
			reject(new Error(`Failed to spawn symbols process: ${error.message}`));
		});
	});
}
