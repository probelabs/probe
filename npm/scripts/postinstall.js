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
import { downloadProbeBinary } from '../src/downloader.js';

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
		// Create the bin directory if it doesn't exist
		console.log(`Creating bin directory at: ${binDir}`);
		await fs.ensureDir(binDir);
		console.log('Bin directory created successfully');

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
		console.log('Created README file in bin directory');

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
		console.log('Created .gitignore file in bin directory');

		// Download the probe binary
		console.log('Downloading probe binary...');
		try {
			// Try to get the package version
			let packageVersion = '0.0.0';
			const possiblePaths = [
				path.resolve(__dirname, '..', 'package.json'),      // When installed from npm: scripts/../package.json
				path.resolve(__dirname, '..', '..', 'package.json') // In development: scripts/../../package.json
			];

			for (const packageJsonPath of possiblePaths) {
				try {
					if (fs.existsSync(packageJsonPath)) {
						console.log(`Found package.json at: ${packageJsonPath}`);
						const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
						if (packageJson.version) {
							packageVersion = packageJson.version;
							console.log(`Using version from package.json: ${packageVersion}`);
							break;
						}
					}
				} catch (err) {
					console.error(`Error reading package.json at ${packageJsonPath}:`, err);
				}
			}

			// Download the binary
			const binaryPath = await downloadProbeBinary(packageVersion);
			console.log(`Successfully downloaded probe binary to: ${binaryPath}`);

			// Get the path to the actual binary (not the probe shim)
			const isWindows = process.platform === 'win32';
			const actualBinaryName = isWindows ? 'probe.exe' : 'probe-binary';
			const actualBinaryPath = path.join(binDir, actualBinaryName);

			// Copy the downloaded binary to the actual binary path
			if (binaryPath !== actualBinaryPath) {
				console.log(`Copying binary from ${binaryPath} to ${actualBinaryPath}`);
				await fs.copyFile(binaryPath, actualBinaryPath);
				await fs.chmod(actualBinaryPath, 0o755); // Make it executable
			}

			console.log('\nProbe binary was successfully downloaded and installed during installation.');
			console.log('You can now use the probe command directly from the command line.');
		} catch (error) {
			console.error('Error downloading probe binary:', error);
			console.error('\nNote: The probe binary will need to be downloaded when you first use the package.');
			console.error('If you encounter any issues, you can manually place the binary in the bin directory.');
			console.error('You can download it from: https://github.com/probelabs/probe/releases');

			// Don't fail the installation, just warn the user
			return;
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