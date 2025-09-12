/**
 * Utility functions for the probe package
 * @module utils
 */

import path from 'path';
import fs from 'fs-extra';
import { fileURLToPath } from 'url';
import { downloadProbeBinary } from './downloader.js';
import { getPackageBinDir } from './directory-resolver.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Note: binDir is now resolved dynamically using getPackageBinDir()

// Store the binary path
let probeBinaryPath = '';

/**
 * Get the path to the probe binary, downloading it if necessary
 * @param {Object} options - Options for getting the binary
 * @param {boolean} [options.forceDownload=false] - Force download even if binary exists
 * @param {string} [options.version] - Specific version to download
 * @returns {Promise<string>} - Path to the binary
 */
export async function getBinaryPath(options = {}) {
	const { forceDownload = false, version } = options;

	// Return cached path if available and not forcing download
	if (probeBinaryPath && !forceDownload && fs.existsSync(probeBinaryPath)) {
		return probeBinaryPath;
	}

	// Check environment variable
	if (process.env.PROBE_PATH && fs.existsSync(process.env.PROBE_PATH) && !forceDownload) {
		probeBinaryPath = process.env.PROBE_PATH;
		return probeBinaryPath;
	}

	// Get dynamic bin directory (handles CI, npx, Docker scenarios)
	const binDir = await getPackageBinDir();
	
	// Check bin directory
	const isWindows = process.platform === 'win32';
	const binaryName = isWindows ? 'probe.exe' : 'probe';
	const binaryPath = path.join(binDir, binaryName);

	if (fs.existsSync(binaryPath) && !forceDownload) {
		probeBinaryPath = binaryPath;
		return probeBinaryPath;
	}

	// Download if not found or force download
	console.log(`${forceDownload ? 'Force downloading' : 'Binary not found. Downloading'} probe binary...`);
	probeBinaryPath = await downloadProbeBinary(version);
	return probeBinaryPath;
}

/**
 * Manually set the path to the probe binary
 * @param {string} binaryPath - Path to the probe binary
 * @throws {Error} If the binary doesn't exist at the specified path
 */
export function setBinaryPath(binaryPath) {
	if (!fs.existsSync(binaryPath)) {
		throw new Error(`No binary found at path: ${binaryPath}`);
	}

	probeBinaryPath = binaryPath;
}

/**
 * Ensure the bin directory exists
 * @returns {Promise<void>}
 */
export async function ensureBinDirectory() {
	// This function is now handled by getPackageBinDir() which ensures directory exists
	// Keeping for backward compatibility but it's no longer needed
	const binDir = await getPackageBinDir();
	await fs.ensureDir(binDir);
}

/**
 * Build command-line arguments from an options object
 * @param {Object} options - Options object
 * @param {Array<string>} flagMap - Map of option keys to command-line flags
 * @returns {Array<string>} - Array of command-line arguments
 */
export function buildCliArgs(options, flagMap) {
	const cliArgs = [];

	for (const [key, flag] of Object.entries(flagMap)) {
		if (key in options) {
			const value = options[key];

			if (typeof value === 'boolean') {
				if (value) {
					cliArgs.push(flag);
				}
			} else if (Array.isArray(value)) {
				for (const item of value) {
					cliArgs.push(flag, item);
				}
			} else if (value !== undefined && value !== null) {
				cliArgs.push(flag, value.toString());
			}
		}
	}

	return cliArgs;
}

/**
 * Escape a string for use in a command line
 * @param {string} str - String to escape
 * @returns {string} - Escaped string
 */
export function escapeString(str) {
  if (process.platform === 'win32') {
    // For Windows PowerShell, escape double quotes and wrap with double quotes
    return `"${str.replace(/"/g, '\\"')}"`;
  } else {
    // Use single quotes for POSIX shells
    // Escape single quotes in the string by replacing ' with '\''
    return `'${str.replace(/'/g, "'\\''")}'`;
  }
}
