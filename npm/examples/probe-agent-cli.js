#!/usr/bin/env node

/**
 * CLI for testing ProbeAgent with different engines
 * Usage: node probe-agent-cli.js [--engine vercel|claude-code] "your question"
 */

import { ProbeAgent } from '../src/agent/ProbeAgent.js';
import chalk from 'chalk';

// Parse command line arguments
const args = process.argv.slice(2);
// Detect engine from environment or default to vercel
let engine = process.env.USE_CLAUDE_CODE === 'true' ? 'claude-code' :
             (process.env.AI_ENGINE || 'vercel');
let question = '';

// Parse --engine flag (overrides environment)
if (args[0] === '--engine' && args[1]) {
  engine = args[1];
  question = args.slice(2).join(' ');
} else {
  question = args.join(' ');
}

// Default question if none provided
if (!question) {
  question = "Find all async functions in this codebase that handle errors";
}

// Color configuration
const colors = {
  engine: chalk.cyan,
  tool: chalk.yellow,
  result: chalk.green,
  error: chalk.red,
  debug: chalk.gray,
  prompt: chalk.magenta,
  text: chalk.white
};

console.log(colors.engine(`\n🚀 Starting Probe Agent with ${engine.toUpperCase()} engine\n`));
console.log(colors.prompt(`Question: "${question}"\n`));

// Check for API key (not required for Claude Code in Claude Code environment)
if (engine === 'claude-code') {
  // Claude Code uses the installed claude command
  console.log(colors.debug('Using Claude Code engine (requires claude command installed)'));
} else if (!process.env.ANTHROPIC_API_KEY && !process.env.OPENAI_API_KEY) {
  console.error(colors.error('⚠️  No API key found. Please set ANTHROPIC_API_KEY or OPENAI_API_KEY'));
  process.exit(1);
}

async function runAgent(engineType) {
  const startTime = Date.now();

  try {
    // Create agent with specified engine
    // For Claude Code, use provider parameter
    // For Vercel, use engineType parameter (backward compat)
    const agentOptions = {
      debug: true, // Enable debug to see tool calls
      allowedFolders: [process.cwd()],
      maxIterations: 5, // Limit iterations for demo
      // Enable streaming to see real-time output
      onStream: (chunk) => {
        // This would show streaming output if implemented
      }
    };

    // Set provider for Claude Code, engineType for others
    if (engineType === 'claude-code') {
      agentOptions.provider = 'claude-code';
    } else {
      agentOptions.engineType = engineType;
    }

    const agent = new ProbeAgent(agentOptions);

    console.log(colors.text('\n📊 Initializing agent...\n'));
    
    // Initialize the agent
    await agent.initialize();

    // Track tool calls
    let toolCallCount = 0;
    if (agent.events) {
      agent.events.on('toolCall', (event) => {
        toolCallCount++;
        if (event.status === 'started') {
          console.log(colors.tool(`\n🔧 Tool ${toolCallCount}: ${event.name}`));
          if (event.args && Object.keys(event.args).length > 0) {
            console.log(colors.debug(`   Args: ${JSON.stringify(event.args, null, 2).substring(0, 200)}...`));
          }
        }
      });
    }

    console.log(colors.text('🤔 Querying agent...\n'));

    // Execute the question
    const response = await agent.answer(question);

    console.log(colors.result('\n✅ Response:\n'));
    console.log(colors.text(response));
    console.log(colors.text('\n' + '─'.repeat(80)));

    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    console.log(colors.debug(`\n⏱️  Duration: ${duration}s`));
    console.log(colors.debug(`🔧 Tools used: ${toolCallCount}`));
    console.log(colors.engine(`🚀 Engine: ${engineType.toUpperCase()}\n`));

  } catch (error) {
    console.error(colors.error('\n❌ Error:'), error.message);
    if (error.stack) {
      console.error(colors.debug(error.stack));
    }
    process.exit(1);
  }
}

// Run the agent
runAgent(engine).catch(error => {
  console.error(colors.error('Fatal error:'), error);
  process.exit(1);
});
