#!/usr/bin/env node

import fs from 'fs-extra';
import path from 'path';
import { fileURLToPath } from 'url';
import { exec } from 'child_process';
import { promisify } from 'util';

// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Path to the bin directory (relative to this script)
const binDir = path.resolve(__dirname, '..', 'bin');

// Create the bin directory if it doesn't exist
console.log(`Creating bin directory at: ${binDir}`);
try {
	fs.ensureDirSync(binDir);
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

You can download the binary from: https://github.com/buger/probe/releases
`;

	fs.writeFileSync(readmePath, readmeContent);
	console.log('Created README file in bin directory');

	// Create a .gitignore file to ignore binaries but keep the directory
	const gitignorePath = path.join(binDir, '.gitignore');
	const gitignoreContent = `# Ignore all files in this directory
*
# Except these files
!.gitignore
!.gitkeep
!README.md
`;

	fs.writeFileSync(gitignorePath, gitignoreContent);
	console.log('Created .gitignore file in bin directory');

} catch (error) {
	console.error(`Error setting up bin directory: ${error.message}`);
	console.error('You may need to manually create the bin directory or run with elevated privileges.');
}

// Download the probe binary
async function downloadBinary() {
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

		// If we still have 0.0.0, try to get version from npm package
		if (packageVersion === '0.0.0') {
			try {
				const execAsync = promisify(exec);
				// Try to get version from the package name itself
				const result = await execAsync('npm list -g @buger/probe-mcp --json');
				const npmList = JSON.parse(result.stdout);
				if (npmList.dependencies && npmList.dependencies['@buger/probe-mcp']) {
					packageVersion = npmList.dependencies['@buger/probe-mcp'].version;
					console.log(`Using version from npm list: ${packageVersion}`);
				}
			} catch (err) {
				console.error('Error getting version from npm:', err);
			}
		}

		// Import the downloader module
		const { downloadProbeBinary } = await import('../build/downloader.js');

		console.log(`Downloading probe binary (version: ${packageVersion})...`);
		const binaryPath = await downloadProbeBinary(packageVersion);
		console.log(`Successfully downloaded probe binary to: ${binaryPath}`);

		// Make sure the binary is executable (on non-Windows platforms)
		if (process.platform !== 'win32') {
			try {
				await fs.chmod(binaryPath, 0o755);
				console.log(`Made binary executable: ${binaryPath}`);
			} catch (err) {
				console.warn(`Warning: Could not set executable permissions on binary: ${err}`);
			}
		}

		return binaryPath;
	} catch (error) {
		console.error('Error downloading probe binary:', error);
		console.error('You can manually download the binary from https://github.com/buger/probe/releases');
		console.error('and place it in the bin directory with the name "probe" (or "probe.exe" on Windows).');

		// Don't fail the installation, just warn the user
		return null;
	}
}

// Execute the download
downloadBinary().then(binaryPath => {
	if (binaryPath) {
		console.log('\nProbe binary was successfully downloaded during installation.');
		console.log('The MCP server will use this binary when it runs.');
	} else {
		console.log('\nNote: The probe binary will need to be downloaded when you first run the MCP server.');
		console.log('If you encounter any issues, you can manually place the binary in the bin directory.');
	}
}).catch(error => {
	console.error('Unexpected error during binary download:', error);
});