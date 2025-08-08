#!/usr/bin/env node

// Simple test to check if MCP server correctly handles search_code method

import { spawn } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Path to the MCP server
const SERVER_PATH = path.join(__dirname, 'build', 'index.js');

console.log('Testing MCP server for search_code method registration...');

// Start the MCP server
const server = spawn('node', [SERVER_PATH], {
	stdio: ['pipe', 'pipe', 'pipe']
});

let output = '';
server.stdout.on('data', (data) => {
	output += data.toString();
});

server.stderr.on('data', (data) => {
	console.error('Server stderr:', data.toString());
});

// Test 1: listTools request
const listToolsRequest = {
	jsonrpc: '2.0',
	id: 1,
	method: 'listTools',
	params: {}
};

// Test 2: callTool request for search_code
const callToolRequest = {
	jsonrpc: '2.0',
	id: 2,
	method: 'callTool',
	params: {
		name: 'search_code',
		arguments: {
			path: process.cwd(),
			query: 'test'
		}
	}
};

function sendMessage(message) {
	const content = JSON.stringify(message);
	const headers = `Content-Length: ${Buffer.byteLength(content, 'utf8')}\r\n\r\n`;
	const fullMessage = headers + content;
	
	console.log('Sending message:', JSON.stringify(message, null, 2));
	server.stdin.write(fullMessage);
}

setTimeout(() => {
	console.log('1. Testing listTools...');
	sendMessage(listToolsRequest);
	
	setTimeout(() => {
		console.log('2. Testing search_code call...');
		sendMessage(callToolRequest);
		
		setTimeout(() => {
			console.log('Server output captured:');
			console.log(output);
			server.kill();
		}, 3000);
	}, 2000);
}, 1000);

server.on('close', (code) => {
	console.log(`Test completed. Server exited with code: ${code}`);
});