#!/usr/bin/env node

/**
 * Post-install script for the probe package
 * 
 * This script is executed after the package is installed.
 * It downloads the probe binary and replaces the placeholder binary with the actual one.
 */

import fs from 'fs-extra';
import path from 'path';
import { fileURLToPath } from 'url';
import { execFile } from 'child_process';
import { promisify } from 'util';
import { extractBundledBinary } from '../src/extractor.js';
import { downloadProbeBinary } from '../src/downloader.js';

const execFileAsync = promisify(execFile);

// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Path to the bin directory (relative to this script)
const binDir = path.resolve(__dirname, '..', 'bin');

/**
 * Main function
 */
async function main() {
	try {
		// Skip postinstall in CI environments to prevent wrong binaries from being packaged
		const isCI = process.env.CI || process.env.GITHUB_ACTIONS || process.env.CONTINUOUS_INTEGRATION || 
		             process.env.BUILD_NUMBER || process.env.JENKINS_URL || process.env.TRAVIS ||
		             process.env.CIRCLECI || process.env.GITLAB_CI;
		
		if (isCI) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log('Detected CI environment, skipping probe binary download');
			}
			return;
		}

		// Skip postinstall if binary already exists (for development)
		const isWindows = process.platform === 'win32';
		const targetBinaryName = isWindows ? 'probe.exe' : 'probe-binary';
		const targetBinaryPath = path.join(binDir, targetBinaryName);
		
		if (await fs.pathExists(targetBinaryPath)) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Probe binary already exists at ${targetBinaryPath}, skipping download`);
			}
			return;
		}

		// Create the bin directory if it doesn't exist
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Creating bin directory at: ${binDir}`);
		}
		await fs.ensureDir(binDir);
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log('Bin directory created successfully');
		}

		// Create a README file in the bin directory
		const readmePath = path.join(binDir, 'README.md');
		const readmeContent = `# Probe Binary Directory

This directory is used to store the downloaded probe binary.

The binary is automatically downloaded during package installation.
If you encounter any issues with the download, you can manually place the probe binary in this directory.

Binary name should be:
- \`probe\` (on Linux/macOS)
- \`probe.exe\` (on Windows)

You can download the binary from: https://github.com/probelabs/probe/releases
`;

		await fs.writeFile(readmePath, readmeContent);
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log('Created README file in bin directory');
		}

		// Create a .gitignore file to ignore binaries but keep the directory
		const gitignorePath = path.join(binDir, '.gitignore');
		const gitignoreContent = `# Ignore all files in this directory
*
# Except these files
!.gitignore
!.gitkeep
!README.md
!probe
`;

		await fs.writeFile(gitignorePath, gitignoreContent);
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log('Created .gitignore file in bin directory');
		}

		// Get the package version first
		let packageVersion = '0.0.0';
		const possiblePaths = [
			path.resolve(__dirname, '..', 'package.json'),      // When installed from npm: scripts/../package.json
			path.resolve(__dirname, '..', '..', 'package.json') // In development: scripts/../../package.json
		];

		for (const packageJsonPath of possiblePaths) {
			try {
				if (fs.existsSync(packageJsonPath)) {
					if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
						console.log(`Found package.json at: ${packageJsonPath}`);
					}
					const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
					if (packageJson.version) {
						packageVersion = packageJson.version;
						if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
							console.log(`Using version from package.json: ${packageVersion}`);
						}
						break;
					}
				}
			} catch (err) {
				console.error(`Error reading package.json at ${packageJsonPath}:`, err);
			}
		}

		// Try to extract bundled binary first
		let binaryPath;
		try {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log('Extracting bundled probe binary...');
			}
			binaryPath = await extractBundledBinary(packageVersion);
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Successfully extracted bundled binary to: ${binaryPath}`);
			}
		} catch (extractError) {
			// If bundled binary extraction fails, fall back to downloading
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Bundled binary extraction failed: ${extractError.message}`);
				console.log('Falling back to downloading binary from GitHub...');
			} else {
				console.log('Bundled binary not found, downloading from GitHub...');
			}

			try {
				binaryPath = await downloadProbeBinary(packageVersion);
				if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
					console.log(`Successfully downloaded probe binary to: ${binaryPath}`);
				}
			} catch (downloadError) {
				throw new Error(
					`Failed to install probe binary.\n` +
					`Bundled extraction error: ${extractError.message}\n` +
					`Download error: ${downloadError.message}\n` +
					`\n` +
					`Please check:\n` +
					`1. Your platform is supported\n` +
					`2. You have internet connectivity\n` +
					`3. GitHub releases are accessible`
				);
			}
		}

		// Copy the extracted/downloaded binary to the correct location if needed
		// (targetBinaryName and targetBinaryPath already declared at the beginning of main function)
			if (binaryPath !== targetBinaryPath) {
				if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
					console.log(`Copying binary to ${targetBinaryPath} from ${binaryPath}`);
				}
				await fs.copyFile(binaryPath, targetBinaryPath);
				await fs.chmod(targetBinaryPath, 0o755); // Make it executable
			}

			// On macOS, try to remove quarantine attributes that might prevent execution
			if (process.platform === 'darwin') {
				try {
					// Security: Use execFile with array args instead of exec to prevent command injection
					// Validate that the path is within the bin directory
					const normalizedPath = path.normalize(targetBinaryPath);
					const normalizedBinDir = path.normalize(binDir);
					if (!normalizedPath.startsWith(normalizedBinDir)) {
						throw new Error('Invalid binary path - outside of bin directory');
					}

					await execFileAsync('xattr', ['-d', 'com.apple.quarantine', normalizedPath]).catch(() => {
						// Ignore errors - xattr may not exist or file may not have quarantine attribute
					});

					if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
						console.log('Removed quarantine attributes from binary');
					}
				} catch (error) {
					// Ignore errors - this is just a precaution
					if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
						console.log('Note: Could not remove quarantine attributes (this is usually fine)');
					}
				}
			}

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log('\nProbe binary was successfully installed.');
			console.log('You can now use the probe command directly from the command line.');
		}
	} catch (error) {
		console.error(`Error in postinstall script: ${error.message}`);
		console.error('You may need to manually create the bin directory or run with elevated privileges.');
	}
}

// Execute the main function
main().catch(error => {
	console.error('Unexpected error during postinstall:', error);
});