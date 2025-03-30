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
import { ProbeChat } from './probeChat.js';
import { TokenUsageDisplay } from './tokenUsageDisplay.js';
import { DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';

/**
 * Main function that runs the Probe Chat CLI or web interface
 */
export function main() {
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
    .option('-f, --force-provider <provider>', 'Force a specific provider (options: anthropic, openai, google)')
    .option('-w, --web', 'Run in web interface mode')
    .option('-p, --port <port>', 'Port to run web server on (default: 8080)')
    .argument('[path]', 'Path to the codebase to search (overrides ALLOWED_FOLDERS)')
    .parse(process.argv);

  const options = program.opts();
  const pathArg = program.args[0];

  if (options.debug) {
    process.env.DEBUG_CHAT = '1';
    console.log(chalk.yellow('Debug mode enabled'));
  }
  if (options.model) {
    process.env.MODEL_NAME = options.model;
    console.log(chalk.blue(`Using model: ${options.model}`));
  }
  if (options.forceProvider) {
    const provider = options.forceProvider.toLowerCase();
    if (!['anthropic', 'openai', 'google'].includes(provider)) {
      console.error(chalk.red(`Error: Invalid provider "${provider}". Must be one of: anthropic, openai, google`));
      process.exit(1);
    }
    process.env.FORCE_PROVIDER = provider;
    console.log(chalk.blue(`Forcing provider: ${provider}`));
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
  const googleApiKey = process.env.GOOGLE_API_KEY;

  // Check if we have at least one API key
  const hasApiKeys = !!(anthropicApiKey || openaiApiKey || googleApiKey);

  // Determine whether to run in CLI or web mode
  if (options.web) {
    if (!hasApiKeys) {
      console.warn(chalk.yellow('Warning: No API key provided. The web interface will show instructions on how to set up API keys.'));
    }
    // Import and start web server
    import('./webServer.js')
      .then(module => {
        const { startWebServer } = module;
        startWebServer(version, hasApiKeys);
      })
      .catch(error => {
        console.error(chalk.red(`Error starting web server: ${error.message}`));
        process.exit(1);
      });
    return;
  }

  // In CLI mode, we need API keys to proceed
  if (!hasApiKeys) {
    console.error(chalk.red('Error: No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.'));
    console.log(chalk.cyan('You can find these instructions in the .env.example file:'));
    console.log(chalk.cyan('1. Create a .env file by copying .env.example'));
    console.log(chalk.cyan('2. Add your API key to the .env file'));
    console.log(chalk.cyan('3. Restart the application'));
    process.exit(1);
  }

  // Initialize ProbeChat for CLI mode
  let chat;
  try {
    chat = new ProbeChat();

    // Print which model is being used
    if (chat.apiType === 'anthropic') {
      console.log(chalk.green(`Using Anthropic API with model: ${chat.model}`));
    } else if (chat.apiType === 'openai') {
      console.log(chalk.green(`Using OpenAI API with model: ${chat.model}`));
    } else if (chat.apiType === 'google') {
      console.log(chalk.green(`Using Google API with model: ${chat.model}`));
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
    // Check if response is a structured object with response and tokenUsage properties
    if (response && typeof response === 'object' && 'response' in response) {
      // Extract the text response
      const textResponse = response.response;

      // Format the text response
      return textResponse.replace(
        /<tool_call>(.*?)<\/tool_call>/gs,
        (match, toolCall) => chalk.magenta(`[Tool Call] ${toolCall}`)
      );
    }

    // Fallback for legacy format (plain string)
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
        const display = new TokenUsageDisplay();
        const formatted = display.format(usage);

        // Current usage badge
        console.log(chalk.blue('Current:', formatted.current.total));
        // Context window
        console.log(chalk.blue('Context:', formatted.contextWindow));
        // Cache information
        console.log(chalk.blue('Cache:',
          `Read: ${formatted.current.cache.read},`,
          `Write: ${formatted.current.cache.write},`,
          `Total: ${formatted.current.cache.total}`));
        // Total usage badge
        console.log(chalk.blue('Total:', formatted.total.total));
        // Show context window in terminal title
        process.stdout.write('\x1B]0;Context: ' + formatted.contextWindow + '\x07');
        continue;
      } else if (message.toLowerCase() === 'clear') {
        const newSessionId = chat.clearHistory();
        console.log(chalk.yellow('Chat history cleared'));
        console.log(chalk.blue(`New session ID: ${newSessionId}`));
        continue;
      }

      const spinner = ora('Thinking...').start();
      try {
        const result = await chat.chat(message);
        spinner.stop();

        console.log(chalk.green('Assistant:'));
        console.log(formatResponse(result));
        console.log();

        // If we have token usage data in the response, update the terminal title with context window size
        if (result && typeof result === 'object' && result.tokenUsage && result.tokenUsage.contextWindow) {
          process.stdout.write('\x1B]0;Context: ' + result.tokenUsage.contextWindow + '\x07');
        }
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

// If this file is run directly, call main()
if (import.meta.url === import.meta.main) {
  main();
}