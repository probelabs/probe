#!/usr/bin/env node

import fs from 'fs-extra';
import path from 'path';
import { fileURLToPath } from 'url';

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

The binary will be automatically downloaded when you run the MCP server for the first time.
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

console.log('\nNote: The probe binary will be downloaded automatically when you first run the MCP server.');
console.log('If you encounter any issues, you can manually place the binary in the bin directory.');