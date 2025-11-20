#!/usr/bin/env node

/**
 * CLI for testing ProbeAgent with different providers
 * Usage: node probe-agent-cli.js [--provider vercel|claude-code] "your question"
 */

import { ProbeAgent } from '../src/agent/ProbeAgent.js';
import chalk from 'chalk';

// Parse command line arguments
const args = process.argv.slice(2);
// Detect provider from environment (claude-code auto-detects if no API keys)
let provider = process.env.USE_CLAUDE_CODE === 'true' ? 'claude-code' : null;
let question = '';

// Parse --provider flag (overrides environment)
if (args[0] === '--provider' && args[1]) {
  provider = args[1];
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
  provider: chalk.cyan,
  tool: chalk.yellow,
  result: chalk.green,
  error: chalk.red,
  debug: chalk.gray,
  prompt: chalk.magenta,
  text: chalk.white
};

const displayProvider = provider || 'auto-detect';
console.log(colors.provider(`\n🚀 Starting Probe Agent (provider: ${displayProvider})\n`));
console.log(colors.prompt(`Question: "${question}"\n`));

// Check for API key (not required for Claude Code)
if (provider === 'claude-code') {
  console.log(colors.debug('Using Claude Code provider (requires claude command installed)'));
} else if (!provider && !process.env.ANTHROPIC_API_KEY && !process.env.OPENAI_API_KEY) {
  console.log(colors.debug('No API keys found - will attempt Claude Code auto-fallback'));
}

async function runAgent(providerName) {
  const startTime = Date.now();

  try {
    // Create agent - provider is optional (auto-detects)
    const agentOptions = {
      debug: true,
      allowedFolders: [process.cwd()],
      maxIterations: 5
    };

    // Set provider if specified (otherwise auto-detect)
    if (providerName) {
      agentOptions.provider = providerName;
    }

    const agent = new ProbeAgent(agentOptions);

    console.log(colors.text('\n📊 Initializing agent...\n'));
    
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

    const response = await agent.answer(question);

    console.log(colors.result('\n✅ Response:\n'));
    console.log(colors.text(response));
    console.log(colors.text('\n' + '─'.repeat(80)));

    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    console.log(colors.debug(`\n⏱️  Duration: ${duration}s`));
    console.log(colors.debug(`🔧 Tools used: ${toolCallCount}`));
    console.log(colors.provider(`🚀 Provider: ${agent.clientApiProvider || 'auto-detected'}\n`));

  } catch (error) {
    console.error(colors.error('\n❌ Error:'), error.message);
    if (error.stack) {
      console.error(colors.debug(error.stack));
    }
    process.exit(1);
  }
}

runAgent(provider).catch(error => {
  console.error(colors.error('Fatal error:'), error);
  process.exit(1);
});
