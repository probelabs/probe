#!/usr/bin/env node
import 'dotenv/config';
import inquirer from 'inquirer';
import chalk from 'chalk';
import ora from 'ora';
import { Command } from 'commander';
import { existsSync, realpathSync } from 'fs';
import { resolve } from 'path';
import { randomUUID } from 'crypto';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { generateText } from 'ai';
// Import tools and system message from @buger/probe
import { tools } from '@buger/probe';

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
  .version('1.0.0')
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

try {
  // Generate a unique session ID
  sessionId = randomUUID();
  console.log(chalk.blue(`Session ID: ${sessionId}`));
  
  // Store the session ID in an environment variable for tools to access
  process.env.PROBE_SESSION_ID = sessionId;
  
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

// Track token usage for monitoring (very approximate)
let totalRequestTokens = 0;
let totalResponseTokens = 0;

// Simple token counter function (very approximate)
function countTokens(text) {
  // Rough approximation: 1 token ≈ 4 characters for English text
  return Math.ceil(text.length / 4);
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
      console.log(chalk.cyan('Token Usage (approximate):'));
      console.log(chalk.cyan(`  Request tokens: ${totalRequestTokens}`));
      console.log(chalk.cyan(`  Response tokens: ${totalResponseTokens}`));
      console.log(chalk.cyan(`  Total tokens: ${totalRequestTokens + totalResponseTokens}`));
      continue;
    } else if (message.toLowerCase() === 'clear') {
      history = [];
      sessionId = randomUUID();
      process.env.PROBE_SESSION_ID = sessionId;
      console.log(chalk.yellow('Chat history cleared'));
      console.log(chalk.blue(`New session ID: ${sessionId}`));
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
        system: customizeSystemMessage(tools.DEFAULT_SYSTEM_MESSAGE), // Customize the system message
        tools: {
          search: tools.searchTool,
          query: tools.queryTool,
          extract: tools.extractTool
        },
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
      
      // Log tool usage
      if (result.toolCalls && result.toolCalls.length > 0) {
        console.log('Tool was used:', result.toolCalls.length, 'times');
        result.toolCalls.forEach((call, index) => {
          console.log(`Tool call ${index + 1}:`, call.name);
        });
      }
      
      // Stop the spinner
      spinner.stop();
      
      // Print the formatted response
      console.log(chalk.green('Assistant:'));
      console.log(formatResponse(result.text));
      console.log(); // Add a blank line for readability
    } catch (error) {
      // Stop the spinner and show the error
      spinner.stop();
      console.error(chalk.red(`Error: ${error.message}`));
    }
  }
}

// Function to customize the system message with allowed folders information
function customizeSystemMessage(systemMessage) {
  if (allowedFolders.length > 0) {
    const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
    return systemMessage + `\n\nThe following folders are configured for code search: ${folderList}. When using search, specify one of these folders in the path argument.`;
  } else {
    return systemMessage + `\n\nNo specific folders are configured for code search, so the current directory will be used by default. You can omit the path parameter in your search calls, or use '.' to explicitly search in the current directory.`;
  }
}

// Start the chat
startChat().catch((error) => {
  console.error(chalk.red(`Fatal error: ${error.message}`));
  process.exit(1);
}); 