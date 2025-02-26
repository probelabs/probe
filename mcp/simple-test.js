#!/usr/bin/env node

// Simple test script to check if the MCP server is running

const { spawn } = require('child_process');
const path = require('path');

// Path to the MCP server
const SERVER_PATH = path.join(__dirname, 'build', 'index.js');

console.log('Starting MCP server...');
console.log(`Server path: ${SERVER_PATH}`);

// Start the MCP server
const server = spawn('node', [SERVER_PATH], {
	stdio: 'inherit'  // This will pipe all stdio directly to the parent process
});

// Set a timeout to kill the server after 5 seconds
setTimeout(() => {
	console.log('Test completed. Killing server...');
	server.kill();
}, 5000);

server.on('close', (code) => {
	console.log(`Server process exited with code ${code}`);
});
