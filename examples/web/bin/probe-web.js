#!/usr/bin/env node

// This script starts the Probe web interface

import path from 'path';
import { fileURLToPath } from 'url';
import { spawn } from 'child_process';
import fs from 'fs';

// Get the directory name of the current module
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, '..');
const mainJsPath = path.join(packageDir, 'main.js');

// Check if main.js exists
if (!fs.existsSync(mainJsPath)) {
	console.error('Error: main.js not found in the package directory');
	process.exit(1);
}

console.log('Starting Probe Web Interface from:', packageDir);

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