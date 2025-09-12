/**
 * Directory resolver for probe binary storage
 * Handles reliable directory resolution across different environments (CI, npx, Docker, etc.)
 * @module directory-resolver
 */

import path from 'path';
import os from 'os';
import fs from 'fs-extra';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/**
 * Get a writable directory for storing the probe binary
 * Tries multiple strategies in order of preference
 * @returns {Promise<string>} Path to writable bin directory
 */
export async function getPackageBinDir() {
	const debug = process.env.DEBUG === '1' || process.env.VERBOSE === '1';
	
	if (debug) {
		console.log('DEBUG: Starting probe binary directory resolution');
	}

	// Strategy 1: Check for explicit binary path override
	if (process.env.PROBE_BINARY_PATH) {
		if (debug) {
			console.log(`DEBUG: Checking PROBE_BINARY_PATH: ${process.env.PROBE_BINARY_PATH}`);
		}
		
		const binaryPath = process.env.PROBE_BINARY_PATH;
		if (await fs.pathExists(binaryPath)) {
			const binDir = path.dirname(binaryPath);
			if (await canWriteToDirectory(binDir)) {
				if (debug) {
					console.log(`DEBUG: Using PROBE_BINARY_PATH directory: ${binDir}`);
				}
				return binDir;
			}
		} else {
			console.warn(`Warning: PROBE_BINARY_PATH ${binaryPath} does not exist`);
		}
	}

	// Strategy 2: Check for cache directory override
	if (process.env.PROBE_CACHE_DIR) {
		if (debug) {
			console.log(`DEBUG: Checking PROBE_CACHE_DIR: ${process.env.PROBE_CACHE_DIR}`);
		}
		
		const cacheDir = path.join(process.env.PROBE_CACHE_DIR, 'bin');
		if (await ensureDirectory(cacheDir)) {
			if (debug) {
				console.log(`DEBUG: Using PROBE_CACHE_DIR: ${cacheDir}`);
			}
			return cacheDir;
		}
	}

	// Strategy 3: Try to find package root and use its bin directory
	const packageRoot = await findPackageRoot();
	if (packageRoot) {
		if (debug) {
			console.log(`DEBUG: Found package root: ${packageRoot}`);
		}
		
		const packageBinDir = path.join(packageRoot, 'bin');
		if (await ensureDirectory(packageBinDir) && await canWriteToDirectory(packageBinDir)) {
			if (debug) {
				console.log(`DEBUG: Using package bin directory: ${packageBinDir}`);
			}
			return packageBinDir;
		} else if (debug) {
			console.log(`DEBUG: Package bin directory ${packageBinDir} not writable, trying fallbacks`);
		}
	}

	// Strategy 4: Use user home directory cache (like Puppeteer)
	const homeCache = path.join(os.homedir(), '.probe', 'bin');
	if (debug) {
		console.log(`DEBUG: Trying home cache directory: ${homeCache}`);
	}
	
	if (await ensureDirectory(homeCache)) {
		if (debug) {
			console.log(`DEBUG: Using home cache directory: ${homeCache}`);
		}
		return homeCache;
	}

	// Strategy 5: Use temp directory as last resort
	const tempCache = path.join(os.tmpdir(), 'probe-cache', 'bin');
	if (debug) {
		console.log(`DEBUG: Trying temp cache directory: ${tempCache}`);
	}
	
	if (await ensureDirectory(tempCache)) {
		if (debug) {
			console.log(`DEBUG: Using temp cache directory: ${tempCache}`);
		}
		return tempCache;
	}

	// If all strategies fail, throw a helpful error
	const errorMessage = [
		'Could not find a writable directory for probe binary.',
		'Tried the following locations:',
		packageRoot ? `- Package bin directory: ${path.join(packageRoot, 'bin')}` : '- Package root not found',
		`- Home cache directory: ${homeCache}`,
		`- Temp cache directory: ${tempCache}`,
		'',
		'You can override the location using environment variables:',
		'- PROBE_BINARY_PATH=/path/to/probe (direct path to binary)',
		'- PROBE_CACHE_DIR=/path/to/cache (cache directory, binary will be in /bin subdirectory)'
	].join('\n');

	throw new Error(errorMessage);
}

/**
 * Find the package root directory by looking for package.json with the correct name
 * @returns {Promise<string|null>} Path to package root or null if not found
 */
async function findPackageRoot() {
	const debug = process.env.DEBUG === '1' || process.env.VERBOSE === '1';
	
	// Start from current module directory
	let currentDir = __dirname;
	const rootDir = path.parse(currentDir).root;

	if (debug) {
		console.log(`DEBUG: Starting package root search from: ${currentDir}`);
	}

	// Walk up until we find package.json with our package name
	while (currentDir !== rootDir) {
		const packageJsonPath = path.join(currentDir, 'package.json');
		
		try {
			if (await fs.pathExists(packageJsonPath)) {
				const packageJson = await fs.readJson(packageJsonPath);
				
				if (debug) {
					console.log(`DEBUG: Found package.json at ${packageJsonPath}, name: ${packageJson.name}`);
				}
				
				// Check if this is our package
				if (packageJson.name === '@probelabs/probe') {
					if (debug) {
						console.log(`DEBUG: Found probe package root: ${currentDir}`);
					}
					return currentDir;
				}
			}
		} catch (err) {
			if (debug) {
				console.log(`DEBUG: Error reading package.json at ${packageJsonPath}: ${err.message}`);
			}
			// Continue searching
		}
		
		currentDir = path.dirname(currentDir);
	}

	if (debug) {
		console.log('DEBUG: Package root not found, reached filesystem root');
	}
	
	return null;
}

/**
 * Ensure a directory exists and is writable
 * @param {string} dirPath - Path to directory
 * @returns {Promise<boolean>} True if directory exists and is writable
 */
async function ensureDirectory(dirPath) {
	const debug = process.env.DEBUG === '1' || process.env.VERBOSE === '1';
	
	try {
		await fs.ensureDir(dirPath);
		
		// Test write permissions by creating a temporary file
		const testFile = path.join(dirPath, '.probe-write-test');
		await fs.writeFile(testFile, 'test');
		await fs.remove(testFile);
		
		if (debug) {
			console.log(`DEBUG: Directory ${dirPath} is writable`);
		}
		
		return true;
	} catch (error) {
		if (debug) {
			console.log(`DEBUG: Directory ${dirPath} not writable: ${error.message}`);
		}
		return false;
	}
}

/**
 * Check if a directory is writable
 * @param {string} dirPath - Path to directory
 * @returns {Promise<boolean>} True if directory is writable
 */
async function canWriteToDirectory(dirPath) {
	const debug = process.env.DEBUG === '1' || process.env.VERBOSE === '1';
	
	try {
		// Check if directory exists
		const exists = await fs.pathExists(dirPath);
		if (!exists) {
			if (debug) {
				console.log(`DEBUG: Directory ${dirPath} does not exist`);
			}
			return false;
		}

		// Test write permissions
		const testFile = path.join(dirPath, '.probe-write-test');
		await fs.writeFile(testFile, 'test');
		await fs.remove(testFile);
		
		if (debug) {
			console.log(`DEBUG: Directory ${dirPath} is writable`);
		}
		
		return true;
	} catch (error) {
		if (debug) {
			console.log(`DEBUG: Directory ${dirPath} not writable: ${error.message}`);
		}
		return false;
	}
}