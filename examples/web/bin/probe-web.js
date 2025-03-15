#!/usr/bin/env node

// This script starts the Probe web interface

import path from 'path';
import { fileURLToPath } from 'url';
import { spawn } from 'child_process';
import fs from 'fs';
import { Command } from 'commander';

// Get the directory name of the current module
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, '..');
const mainJsPath = path.join(packageDir, 'main.js');
const packageJsonPath = path.join(packageDir, 'package.json');

// Read package.json to get the version
let version = '1.0.0'; // Default fallback version
try {
	const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
	version = packageJson.version || version;
} catch (error) {
	console.warn(`Warning: Could not read version from package.json: ${error.message}`);
}

// Create a new instance of the program
const program = new Command();

// Configure the program
program
	.name('probe-web')
	.description('Web interface for Probe code search')
	.version(version)
	.option('-p, --port <port>', 'Port to run the server on (default: 8080)')
	.option('-d, --debug', 'Enable debug mode')
	.parse(process.argv);

// Get the options
const options = program.opts();

// Set debug mode if specified
if (options.debug) {
	process.env.DEBUG = 'true';
	console.log('Debug mode enabled');
}

// Set port if specified
if (options.port) {
	process.env.PORT = options.port;
	console.log(`Using port: ${options.port}`);
}

// Check if main.js exists
if (!fs.existsSync(mainJsPath)) {
	console.error('Error: main.js not found in the package directory');
	process.exit(1);
}

console.log('Starting Probe Web Interface from:', packageDir);
console.log(`Version: ${version}`);

// Start the web server
const server = spawn('node', [mainJsPath], {
	cwd: packageDir,
	stdio: 'inherit',
	env: {
		...process.env,
		PROBE_WEB_INTERFACE: 'true'
	}
});

// Handle server process events
server.on('error', (err) => {
	console.error('Failed to start web server:', err);
	process.exit(1);
});

server.on('close', (code) => {
	console.log(`Web server process exited with code ${code}`);
	process.exit(code);
});

// Handle termination signals
process.on('SIGINT', () => {
	console.log('Received SIGINT. Shutting down web server...');
	server.kill('SIGINT');
});

process.on('SIGTERM', () => {
	console.log('Received SIGTERM. Shutting down web server...');
	server.kill('SIGTERM');
});