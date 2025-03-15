#!/usr/bin/env node

// This script starts the Probe web interface

import path from 'path';
import { fileURLToPath } from 'url';
import { spawn } from 'child_process';
import fs from 'fs';

// Get the directory name of the current module
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(__dirname, '../..');

// Check if main.js exists
if (!fs.existsSync(path.join(webDir, 'main.js'))) {
	console.error('Error: main.js not found in the web directory');
	process.exit(1);
}

console.log('Starting Probe Web Interface from:', webDir);

// Start the web server
const server = spawn('node', ['main.js'], {
	cwd: webDir,
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