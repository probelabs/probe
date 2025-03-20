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
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { generateText } from 'ai';
// Import tool generators and utilities from @buger/probe
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE, listFilesByLevel } from '@buger/probe';

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
  console.warn('No folders configured. Set ALLOWED_FOLDERS in .env file or provide a path argument.');
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

// Check for API keys
const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
const openaiApiKey = process.env.OPENAI_API_KEY;

if (!anthropicApiKey && !openaiApiKey) {
  console.error(chalk.red('Error: No API key provided. Please set ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable.'));
  process.exit(1);
}

// Initialize API provider and model
let apiProvider;
let defaultModel;
let apiType;
let sessionId;
let configuredTools;

try {
  // Generate a unique session ID
  sessionId = randomUUID();
  console.log(chalk.blue(`Session ID: ${sessionId}`));

  // Configure tools with the session ID
  const configOptions = {
    sessionId,
    debug: process.env.DEBUG === 'true' || process.env.DEBUG === '1'
  };

  // Create configured tool instances
  configuredTools = {
    search: searchTool(configOptions),
    query: queryTool(configOptions),
    extract: extractTool(configOptions)
  };

  if (anthropicApiKey) {
    // Initialize Anthropic provider with API key and custom URL if provided
    const anthropicApiUrl = process.env.ANTHROPIC_API_URL || 'https://api.anthropic.com/v1';
    apiProvider = createAnthropic({
      apiKey: anthropicApiKey,
      baseURL: anthropicApiUrl,
    });
    defaultModel = process.env.MODEL_NAME || 'claude-3-7-sonnet-latest';
    apiType = 'anthropic';

    console.log(chalk.green(`Using Anthropic API with model: ${defaultModel}`));
  } else if (openaiApiKey) {
    // Initialize OpenAI provider with API key and custom URL if provided
    const openaiApiUrl = process.env.OPENAI_API_URL || 'https://api.openai.com/v1';
    apiProvider = createOpenAI({
      apiKey: openaiApiKey,
      baseURL: openaiApiUrl,
    });
    defaultModel = process.env.MODEL_NAME || 'gpt-4o-2024-05-13';
    apiType = 'openai';

    console.log(chalk.green(`Using OpenAI API with model: ${defaultModel}`));
  }

  console.log(chalk.cyan('Type "exit" or "quit" to end the chat'));
  console.log(chalk.cyan('Type "usage" to see token usage statistics'));
  console.log(chalk.cyan('Type "clear" to clear the chat history'));
  console.log(chalk.cyan('-------------------------------------------'));
} catch (error) {
  console.error(chalk.red(`Error initializing chat: ${error.message}`));
  process.exit(1);
}

// Track token usage for monitoring
let totalRequestTokens = 0;
let totalResponseTokens = 0;
let toolTokenUsage = {
  request: 0,
  response: 0
};

// Import tiktoken at the top level
import { get_encoding } from 'tiktoken';

// Initialize tokenizer
let tokenizer;
try {
  tokenizer = get_encoding('cl100k_base');
} catch (error) {
  console.warn('Could not initialize tiktoken, falling back to approximate token counting');
}

// Token counter function using tiktoken if available
function countTokens(text) {
  if (tokenizer) {
    try {
      return tokenizer.encode(text).length;
    } catch (error) {
      // Fallback to a simple approximation (1 token ≈ 4 characters)
      return Math.ceil(text.length / 4);
    }
  } else {
    // Fallback to a simple approximation (1 token ≈ 4 characters)
    return Math.ceil(text.length / 4);
  }
}

// Function to extract token usage from tool results
function extractTokenUsage(result) {
  if (typeof result === 'string') {
    // Try to extract token usage information from the result string
    const tokenUsageMatch = result.match(/Token Usage:\s+Request tokens: (\d+)\s+Response tokens: (\d+)\s+Total tokens: (\d+)/);
    if (tokenUsageMatch) {
      return {
        request: parseInt(tokenUsageMatch[1], 10),
        response: parseInt(tokenUsageMatch[2], 10),
        total: parseInt(tokenUsageMatch[3], 10)
      };
    }
  }
  return null;
}

// Function to format the AI response
function formatResponse(response) {
  // Replace tool calls with colored versions
  return response.replace(
    /<tool_call>(.*?)<\/tool_call>/gs,
    (match, toolCall) => chalk.magenta(`[Tool Call] ${toolCall}`)
  );
}

// Chat history
let history = [];

// Maximum number of messages to keep in history
const MAX_HISTORY_MESSAGES = 20;

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
      // Calculate total tokens including tool usage
      const totalRequest = totalRequestTokens + toolTokenUsage.request;
      const totalResponse = totalResponseTokens + toolTokenUsage.response;
      const total = totalRequest + totalResponse;

      console.log(chalk.cyan('Token Usage:'));
      console.log(chalk.cyan(`  Request tokens: ${totalRequest}`));
      console.log(chalk.cyan(`  Response tokens: ${totalResponse}`));
      console.log(chalk.cyan(`  Total tokens: ${total}`));

      if (toolTokenUsage.request > 0 || toolTokenUsage.response > 0) {
        console.log(chalk.cyan('\nTool Usage Breakdown:'));
        console.log(chalk.cyan(`  Tool request tokens: ${toolTokenUsage.request}`));
        console.log(chalk.cyan(`  Tool response tokens: ${toolTokenUsage.response}`));
        console.log(chalk.cyan(`  Tool total tokens: ${toolTokenUsage.request + toolTokenUsage.response}`));
      }

      continue;
    } else if (message.toLowerCase() === 'clear') {
      history = [];
      // Reset token usage
      totalRequestTokens = 0;
      totalResponseTokens = 0;
      toolTokenUsage = {
        request: 0,
        response: 0
      };
      sessionId = randomUUID();
      console.log(chalk.yellow('Chat history cleared'));
      console.log(chalk.blue(`New session ID: ${sessionId}`));

      // Reconfigure tools with the new session ID
      const configOptions = {
        sessionId,
        debug: process.env.DEBUG === 'true' || process.env.DEBUG === '1'
      };

      // Create new configured tool instances
      configuredTools = {
        search: searchTool(configOptions),
        query: queryTool(configOptions),
        extract: extractTool(configOptions)
      };

      continue;
    }

    // Show a spinner while waiting for the response
    const spinner = ora('Thinking...').start();

    try {
      // Count tokens in the user message
      const messageTokens = countTokens(message);
      totalRequestTokens += messageTokens;

      // Limit history to prevent token overflow
      if (history.length > MAX_HISTORY_MESSAGES) {
        const historyStart = history.length - MAX_HISTORY_MESSAGES;
        history = history.slice(historyStart);
      }

      // Prepare messages array
      const messages = [
        ...history,
        { role: 'user', content: message }
      ];

      // Configure generateText options
      const generateOptions = {
        model: apiProvider(defaultModel),
        messages: messages,
        system: await customizeSystemMessage(DEFAULT_SYSTEM_MESSAGE), // Customize the system message
        tools: [configuredTools.search, configuredTools.query, configuredTools.extract],
        maxSteps: 15,
        temperature: 0.7
      };

      // Add API-specific options
      if (apiType === 'anthropic' && defaultModel.includes('3-7')) {
        generateOptions.experimental_thinking = {
          enabled: true,
          budget: 8000
        };
      }

      // Generate response
      const result = await generateText(generateOptions);

      // Add the response to history
      history.push({ role: 'user', content: message });
      history.push({ role: 'assistant', content: result.text });

      // Count tokens in the response
      const responseTokens = countTokens(result.text);
      totalResponseTokens += responseTokens;

      // Log tool usage and extract token information
      if (result.toolCalls && result.toolCalls.length > 0) {
        console.log('Tool was used:', result.toolCalls.length, 'times');

        result.toolCalls.forEach((call, index) => {
          console.log(`Tool call ${index + 1}:`, call.name);

          // Extract token usage from tool results
          if (call.result) {
            const tokenUsage = extractTokenUsage(call.result);
            if (tokenUsage) {
              // Add to tool token usage
              toolTokenUsage.request += tokenUsage.request;
              toolTokenUsage.response += tokenUsage.response;

              if (process.env.DEBUG === 'true' || process.env.DEBUG === '1') {
                console.log(chalk.gray(`  Tool ${call.name} token usage: ${tokenUsage.request} req, ${tokenUsage.response} resp, ${tokenUsage.total} total`));
              }
            }
          }
        });
      }

      // Stop the spinner
      spinner.stop();

      // Print the formatted response
      console.log(chalk.green('Assistant:'));
      console.log(formatResponse(result.text));
      console.log(); // Add a blank line for readability

      // Display token usage after each response
      const responseTokensFormatted = responseTokens.toLocaleString();
      const totalRequestFormatted = (totalRequestTokens + toolTokenUsage.request).toLocaleString();
      const totalResponseFormatted = (totalResponseTokens + toolTokenUsage.response).toLocaleString();
      const totalTokensFormatted = (totalRequestTokens + totalResponseTokens + toolTokenUsage.request + toolTokenUsage.response).toLocaleString();

      console.log(chalk.gray(`Token Usage: ${responseTokensFormatted} (this response) | ${totalTokensFormatted} (total)`));

      // Show detailed breakdown in debug mode
      if (process.env.DEBUG === 'true' || process.env.DEBUG === '1') {
        console.log(chalk.gray(`  Request: ${totalRequestFormatted} | Response: ${totalResponseFormatted}`));
        if (toolTokenUsage.request > 0 || toolTokenUsage.response > 0) {
          console.log(chalk.gray(`  Tool usage: ${(toolTokenUsage.request + toolTokenUsage.response).toLocaleString()}`));
        }
      }

      console.log(); // Add another blank line after token usage
    } catch (error) {
      // Stop the spinner and show the error
      spinner.stop();
      console.error(chalk.red(`Error: ${error.message}`));
    }
  }
}

// Function to customize the system message with allowed folders information and file list
async function customizeSystemMessage(systemMessage) {
  let customizedMessage = systemMessage || DEFAULT_SYSTEM_MESSAGE;

  // Add folder information
  if (allowedFolders.length > 0) {
    const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
    customizedMessage += `\n\nThe following folders are configured for code search: ${folderList}. When using search, specify one of these folders in the path argument.`;
  } else {
    customizedMessage += `\n\nNo specific folders are configured for code search, so the current directory will be used by default. You can omit the path parameter in your search calls, or use '.' to explicitly search in the current directory.`;
  }

  // Add file list information
  try {
    const searchDirectory = allowedFolders.length > 0 ? allowedFolders[0] : '.';
    console.log(`Generating file list for ${searchDirectory}...`);

    const files = await listFilesByLevel({
      directory: searchDirectory,
      maxFiles: 100,
      respectGitignore: true
    });

    if (files.length > 0) {
      customizedMessage += `\n\nHere is a list of up to 100 files in the codebase (organized by directory depth):\n\n`;
      customizedMessage += files.map(file => `- ${file}`).join('\n');
    }

    console.log(`Added ${files.length} files to system message`);
  } catch (error) {
    console.warn(`Warning: Could not generate file list: ${error.message}`);
  }

  return customizedMessage;
}

// Start the chat
startChat().catch((error) => {
  console.error(chalk.red(`Fatal error: ${error.message}`));
  process.exit(1);
});