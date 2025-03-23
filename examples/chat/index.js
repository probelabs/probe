#!/usr/bin/env node
import 'dotenv/config';
import inquirer from 'inquirer';
import chalk from 'chalk';
import ora from 'ora';
import { Command } from 'commander';
import { existsSync, realpathSync, readFileSync } from 'fs';
import { resolve, dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { randomUUID } from 'crypto';
import { ProbeChat } from './probeChat.js'; // Adjust path if needed

// Import tool generators and utilities from @buger/probe (optional)
import { DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';

// Get the directory name of the current module
const __dirname = dirname(fileURLToPath(import.meta.url));
const packageJsonPath = join(__dirname, 'package.json');

// Read package.json to get the version
let version = '1.0.0'; // Default fallback version
try {
  const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
  version = packageJson.version || version;
} catch (error) {
  console.warn(`Warning: Could not read version from package.json: ${error.message}`);
}

// Parse and validate allowed folders from environment variable
const allowedFolders = process.env.ALLOWED_FOLDERS
  ? process.env.ALLOWED_FOLDERS.split(',').map(folder => folder.trim()).filter(Boolean)
  : [];

console.log('Configured search folders:');
for (const folder of allowedFolders) {
  const exists = existsSync(folder);
  console.log(`- ${folder} ${exists ? '✓' : '✗ (not found)'}`);
  if (!exists) {
    console.warn(`Warning: Folder "${folder}" does not exist or is not accessible`);
  }
}

if (allowedFolders.length === 0) {
  console.warn('No folders configured. Set ALLOWED_FOLDERS in .env file or provide a path argument.');
}

// Create a new instance of the program
const program = new Command();

program
  .name('probe-chat')
  .description('CLI chat interface for Probe code search')
  .version(version)
  .option('-d, --debug', 'Enable debug mode')
  .option('-m, --model <model>', 'Specify the model to use')
  .option('-w, --web', 'Run in web interface mode')
  .option('-p, --port <port>', 'Port to run web server on (default: 8080)')
  .argument('[path]', 'Path to the codebase to search (overrides ALLOWED_FOLDERS)')
  .parse(process.argv);

const options = program.opts();
const pathArg = program.args[0];

if (options.debug) {
  process.env.DEBUG = 'true';
  console.log(chalk.yellow('Debug mode enabled'));
}
if (options.model) {
  process.env.MODEL_NAME = options.model;
  console.log(chalk.blue(`Using model: ${options.model}`));
}

// Resolve path argument to override ALLOWED_FOLDERS
if (pathArg) {
  const resolvedPath = resolve(pathArg);
  if (existsSync(resolvedPath)) {
    const realPath = realpathSync(resolvedPath);
    process.env.ALLOWED_FOLDERS = realPath;
    console.log(chalk.blue(`Using codebase path: ${realPath}`));
  } else {
    console.error(chalk.red(`Error: Path does not exist: ${resolvedPath}`));
    process.exit(1);
  }
}

// Set port for web server if specified
if (options.port) {
  process.env.PORT = options.port;
}

// Check for API keys
const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
const openaiApiKey = process.env.OPENAI_API_KEY;
if (!anthropicApiKey && !openaiApiKey) {
  console.error(chalk.red('Error: No API key provided. Please set ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable.'));
  process.exit(1);
}

// Determine whether to run in CLI or web mode
if (options.web) {
  // Run in web mode
  import('./webServer.js')
    .then(module => {
      const { startWebServer } = module;
      startWebServer(version);
    })
    .catch(error => {
      console.error(chalk.red(`Error starting web server: ${error.message}`));
      process.exit(1);
    });
} else {
  // Run in CLI mode
  // Initialize the ProbeChat
  let chat;
  try {
    chat = new ProbeChat();

    // Print which model is being used
    if (chat.apiType === 'anthropic') {
      console.log(chalk.green(`Using Anthropic API with model: ${chat.model}`));
    } else {
      console.log(chalk.green(`Using OpenAI API with model: ${chat.model}`));
    }

    console.log(chalk.blue(`Session ID: ${chat.getSessionId()}`));
    console.log(chalk.cyan('Type "exit" or "quit" to end the chat'));
    console.log(chalk.cyan('Type "usage" to see token usage statistics'));
    console.log(chalk.cyan('Type "clear" to clear the chat history'));
    console.log(chalk.cyan('-------------------------------------------'));
  } catch (error) {
    console.error(chalk.red(`Error initializing chat: ${error.message}`));
    process.exit(1);
  }

  // Format AI response
  function formatResponse(response) {
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

      const spinner = ora('Thinking...').start();
      try {
        const response = await chat.chat(message);
        spinner.stop();

        console.log(chalk.green('Assistant:'));
        console.log(formatResponse(response));
        console.log();
      } catch (error) {
        spinner.stop();
        console.error(chalk.red(`Error: ${error.message}`));
      }
    }
  }

  startChat().catch((error) => {
    console.error(chalk.red(`Fatal error: ${error.message}`));
    process.exit(1);
  });
}