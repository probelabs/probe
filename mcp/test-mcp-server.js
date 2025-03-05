#!/usr/bin/env node

// Test script for the probe MCP server using proper JSON-RPC format

const { spawn } = require('child_process');
const path = require('path');

// Path to the MCP server
const SERVER_PATH = path.join(__dirname, 'build', 'index.js');

// Define JSON-RPC request for listing tools
const LIST_TOOLS_REQUEST = {
	jsonrpc: '2.0',
	id: 1,
	method: 'listTools',
	params: {}
};

// Define JSON-RPC request for calling the search_code tool
const SEARCH_CODE_REQUEST = {
	jsonrpc: '2.0',
	id: 2,
	method: 'callTool',
	params: {
		name: 'search_code',
		arguments: {
			path: process.cwd(),
			query: 'search',
			maxResults: 5
		}
	}
};

// Function to send a JSON-RPC request to the MCP server
function sendRequest(server, request) {
	const content = JSON.stringify(request);
	const message = `Content-Length: ${Buffer.byteLength(content, 'utf8')}\r\n\r\n${content}`;
	server.stdin.write(message);
}

// Start the MCP server
console.log('Starting MCP server test...');
const server = spawn('node', [SERVER_PATH], {
	stdio: ['pipe', 'pipe', 'pipe']
});

// Handle server output
let responseBuffer = '';
let contentLength = null;

server.stdout.on('data', (data) => {
	const chunk = data.toString();
	responseBuffer += chunk;

	// Parse the response
	while (responseBuffer.length > 0) {
		if (contentLength === null) {
			const headerMatch = responseBuffer.match(/Content-Length: (\d+)\r\n\r\n/);
			if (headerMatch) {
				contentLength = parseInt(headerMatch[1], 10);
				responseBuffer = responseBuffer.substring(headerMatch[0].length);
			} else {
				// Incomplete header, wait for more data
				break;
			}
		}

		if (contentLength !== null && responseBuffer.length >= contentLength) {
			const content = responseBuffer.substring(0, contentLength);
			responseBuffer = responseBuffer.substring(contentLength);
			contentLength = null;

			try {
				const response = JSON.parse(content);
				console.log('Received response:', JSON.stringify(response, null, 2));
			} catch (error) {
				console.error('Error parsing response:', error);
			}
		} else {
			// Incomplete message, wait for more data
			break;
		}
	}
});

server.stderr.on('data', (data) => {
	console.error(`stderr: ${data.toString()}`);
});

// Add a timeout to ensure we don't hang indefinitely
const timeout = setTimeout(() => {
	console.log('Test timed out after 10 seconds');
	server.kill();
	process.exit(1);
}, 10000);

server.on('close', (code) => {
	console.log(`Server process exited with code ${code}`);
});

// Send the requests with a delay to ensure the server is ready
setTimeout(() => {
	console.log('1. Testing listTools request...');
	sendRequest(server, LIST_TOOLS_REQUEST);

	setTimeout(() => {
		console.log('2. Testing callTool request for search_code...');
		sendRequest(server, SEARCH_CODE_REQUEST);

		// Give some time for the response and then exit
		setTimeout(() => {
			console.log('Test completed.');
			server.kill();
		}, 5000);
	}, 2000);
}, 1000);
