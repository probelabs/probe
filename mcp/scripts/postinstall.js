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

This directory is used by the MCP server.

The probe binary is now handled by the @buger/probe package, which is a dependency of this package.
You don't need to manually download the binary anymore.

If you encounter any issues, please check that @buger/probe is properly installed.
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

// Check if the probe package is installed
async function checkProbePackage() {
	try {
		// Try to import the probe package
		try {
			const probe = await import('@buger/probe');
			console.log('Successfully imported @buger/probe package');
			
			// Get the binary path from the probe package
			const binaryPath = probe.getBinaryPath();
			console.log(`Probe binary path from package: ${binaryPath}`);
			
			// Check if the binary exists
			if (fs.existsSync(binaryPath)) {
				console.log('Probe binary exists and is ready to use');
				return true;
			} else {
				console.log('Probe binary does not exist yet, it will be downloaded when needed');
				return false;
			}
		} catch (importError) {
			console.error('Error importing @buger/probe package:', importError);
			console.error('Make sure @buger/probe is installed as a dependency');
			return false;
		}
	} catch (error) {
		console.error('Error checking probe package:', error);
		console.error('You may need to manually install the @buger/probe package');
		return false;
	}
}

// Execute the check
checkProbePackage().then(isInstalled => {
	if (isInstalled) {
		console.log('\nProbe package is successfully installed and ready to use.');
		console.log('The MCP server will use this package when it runs.');
	} else {
		console.log('\nNote: The @buger/probe package may need to be installed or configured.');
		console.log('If you encounter any issues, please check your dependencies.');
	}
}).catch(error => {
	console.error('Unexpected error during package check:', error);
});