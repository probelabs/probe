import 'dotenv/config';
import { createServer } from 'http';
import { createAnthropic } from '@ai-sdk/anthropic';
import { streamText, generateText } from 'ai';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';
import { probeTool } from './probeTool.js';

// Initialize Anthropic provider with API key from environment variable
const anthropic = createAnthropic({
	apiKey: process.env.ANTHROPIC_API_KEY,
});

// Parse and validate allowed folders from environment variable
const allowedFolders = process.env.ALLOWED_FOLDERS
	? process.env.ALLOWED_FOLDERS.split(',').map(folder => folder.trim()).filter(Boolean)
	: [];

// Validate folders exist on startup
console.log('Configured search folders:');
for (const folder of allowedFolders) {
	const exists = existsSync(folder);
	console.log(`- ${folder} ${exists ? '✓' : '✗ (not found)'}`);
	if (!exists) {
		console.warn(`Warning: Folder "${folder}" does not exist or is not accessible`);
	}
}

if (allowedFolders.length === 0) {
	console.warn('No folders configured. Set ALLOWED_FOLDERS in .env file.');
}

const server = createServer(async (req, res) => {
	// Serve the HTML file for GET requests to "/"
	if (req.method === 'GET' && req.url === '/') {
		res.writeHead(200, { 'Content-Type': 'text/html' });
		const html = readFileSync('./index.html', 'utf8');
		res.end(html);
		return;
	}

	// Handle GET requests to "/folders" to provide allowed folders to client
	if (req.method === 'GET' && req.url === '/folders') {
		res.writeHead(200, { 'Content-Type': 'application/json' });
		res.end(JSON.stringify({ folders: allowedFolders }));
		return;
	}

	// Handle POST requests to "/chat" for chat functionality
	if (req.method === 'POST' && req.url === '/chat') {
		let body = '';
		req.on('data', chunk => body += chunk);
		req.on('end', async () => {
			try {
				const { message } = JSON.parse(body);

				// Prepare system message with folder context
				let systemMessage = 'You are a helpful assistant that can search code repositories, and answer user questions in details. Where relevant, include diagrams in mermaid format.';

				if (allowedFolders.length > 0) {
					const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
					systemMessage += ` The following folders are configured for code search: ${folderList}. You can use the searchCode tool to search through these folders. When using searchCode, try to have more general queries and simpler queries. For example, instead of 'rpc layer implementation', just use 'rpc'. Avoid unnecessary verbs or nouns, and focus on main keywords. E.g. no need to use words like layer, or implementation - you should use keywords which could be found in code. For the folder argument use only allowed folders. Only 1 directory per call. When you have distinct terms, do the separate queries for them.`;
				}

				// Create messages array with user's message
				const messages = [
					{
						role: 'user',
						content: message
					}
				];

				console.log('Sending message to Claude with tool support');

				res.writeHead(200, {
					'Content-Type': 'text/plain',
					'Transfer-Encoding': 'chunked',
					'Cache-Control': 'no-cache',
					'Connection': 'keep-alive'
				});

				// Use streamText with tools support
				try {
					const result = await streamText({
						model: anthropic('claude-3-7-sonnet-latest'),
						messages: messages,
						system: systemMessage,
						tools: {
							searchCode: probeTool
						},
						experimental_thinking: {
							enabled: true,           // Enable thinking mode
							budget: 5000             // Set thinking budget in tokens (adjust as needed)
						},
						maxSteps: 10,
						temperature: 0.7
					});

					// Stream the response chunks
					for await (const chunk of result.textStream) {
						res.write(chunk);
					}

					// Handle the final result after streaming completes
					const finalResult = await result;

					// Log tool usage
					if (finalResult.toolCalls && finalResult.toolCalls.length > 0) {
						console.log('Tool was used:', finalResult.toolCalls.length, 'times');
						finalResult.toolCalls.forEach((call, index) => {
							console.log(`Tool call ${index + 1}:`, call.name);
						});
					}

					res.end();
					console.log('Finished streaming response');
				} catch (error) {
					console.error('Error streaming response:', error);
					res.writeHead(500, { 'Content-Type': 'text/plain' });
					res.end('Error generating response');
				}
			} catch (error) {
				console.error(error);
				res.writeHead(500, { 'Content-Type': 'text/plain' });
				res.end('Internal Server Error');
			}
		});
		return;
	}

	// Handle 404 for other routes
	res.writeHead(404, { 'Content-Type': 'text/plain' });
	res.end('Not Found');
});

// Start the server
const PORT = process.env.PORT || 3000;
server.listen(PORT, () => {
	console.log(`Server running on http://localhost:${PORT}`);
	console.log(`Environment: ${process.env.NODE_ENV || 'development'}`);
	console.log('Probe tool is available for AI to use');
});