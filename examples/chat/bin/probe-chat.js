#!/usr/bin/env node

/**
 * @buger/probe-chat CLI
 * Command-line interface for Probe code search chat
 */

import 'dotenv/config';
import inquirer from 'inquirer';
import chalk from 'chalk';
import ora from 'ora';
import { Command } from 'commander';
import { existsSync, realpathSync, readFileSync } from 'fs';
import { resolve, dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { ProbeChat } from '../probeChat.js';

// Get the directory name of the current module
const __dirname = dirname(fileURLToPath(import.meta.url));
const packageDir = resolve(__dirname, '..');
const packageJsonPath = join(packageDir, 'package.json');

// Read package.json to get the version
let version = '1.0.0'; // Default fallback version
try {
	const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
	version = packageJson.version || version;
} catch (error) {
	console.warn(`Warning: Could not read version from package.json: ${error.message}`);
}

// Create a new instance of the program
const program = new Command();

// Configure the program
program
	.name('probe-chat')
	.description('CLI chat interface for Probe code search')
	.version(version)
	.option('-d, --debug', 'Enable debug mode')
	.option('-m, --model <model>', 'Specify the model to use')
	.argument('[path]', 'Path to the codebase to search (overrides ALLOWED_FOLDERS)')
	.parse(process.argv);

// Get the options
const options = program.opts();

// Get the path argument
const pathArg = program.args[0];

// Set debug mode if specified
if (options.debug) {
	process.env.DEBUG = 'true';
	console.log(chalk.yellow('Debug mode enabled'));
}

// Set model if specified
if (options.model) {
	process.env.MODEL_NAME = options.model;
	console.log(chalk.blue(`Using model: ${options.model}`));
}

// Set ALLOWED_FOLDERS if path is provided
if (pathArg) {
	const resolvedPath = resolve(pathArg);

	// Check if the path exists
	if (existsSync(resolvedPath)) {
		// Get the real path (resolves symlinks)
		const realPath = realpathSync(resolvedPath);

		// Set the ALLOWED_FOLDERS environment variable
		process.env.ALLOWED_FOLDERS = realPath;
		console.log(chalk.blue(`Using codebase path: ${realPath}`));
	} else {
		console.error(chalk.red(`Error: Path does not exist: ${resolvedPath}`));
		process.exit(1);
	}
}

// Initialize the chat
let chat;
try {
	chat = new ProbeChat();

	// Print the model being used
	if (chat.apiType === 'anthropic') {
		console.log(chalk.green(`Using Anthropic API with model: ${chat.model}`));
	} else {
		console.log(chalk.green(`Using OpenAI API with model: ${chat.model}`));
	}

	// Print the session ID
	console.log(chalk.blue(`Session ID: ${chat.getSessionId()}`));

	console.log(chalk.cyan('Type "exit" or "quit" to end the chat'));
	console.log(chalk.cyan('Type "usage" to see token usage statistics'));
	console.log(chalk.cyan('Type "clear" to clear the chat history'));
	console.log(chalk.cyan('-------------------------------------------'));
} catch (error) {
	console.error(chalk.red(`Error initializing chat: ${error.message}`));
	process.exit(1);
}

// Function to format the AI response
function formatResponse(response) {
	// Replace tool calls with colored versions
	return response.replace(
		/<tool_call>(.*?)<\/tool_call>/gs,
		(match, toolCall) => chalk.magenta(`[Tool Call] ${toolCall}`)
	);
}

// Main chat loop
async function startChat() {
	while (true) {
		const { message } = await inquirer.prompt([
			{
				type: 'input',
				name: 'message',
				message: chalk.blue('You:'),
				prefix: '',
			},
		]);

		// Handle special commands
		if (message.toLowerCase() === 'exit' || message.toLowerCase() === 'quit') {
			console.log(chalk.yellow('Goodbye!'));
			break;
		} else if (message.toLowerCase() === 'usage') {
			const usage = chat.getTokenUsage();
			console.log(chalk.cyan('Token Usage:'));
			console.log(chalk.cyan(`  Request tokens: ${usage.request}`));
			console.log(chalk.cyan(`  Response tokens: ${usage.response}`));
			console.log(chalk.cyan(`  Total tokens: ${usage.total}`));
			continue;
		} else if (message.toLowerCase() === 'clear') {
			const newSessionId = chat.clearHistory();
			console.log(chalk.yellow('Chat history cleared'));
			console.log(chalk.blue(`New session ID: ${newSessionId}`));
			continue;
		}

		// Show a spinner while waiting for the response
		const spinner = ora('Thinking...').start();

		try {
			// Get response from the chat
			const response = await chat.chat(message);

			// Stop the spinner
			spinner.stop();

			// Print the formatted response
			console.log(chalk.green('Assistant:'));
			console.log(formatResponse(response));
			console.log(); // Add a blank line for readability
		} catch (error) {
			// Stop the spinner and show the error
			spinner.stop();
			console.error(chalk.red(`Error: ${error.message}`));
		}
	}
}

// Start the chat
startChat().catch((error) => {
	console.error(chalk.red(`Fatal error: ${error.message}`));
	process.exit(1);
});