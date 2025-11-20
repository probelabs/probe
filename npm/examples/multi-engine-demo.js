#!/usr/bin/env node

/**
 * Demonstration of ProbeAgent with Claude Code integration
 * Shows auto-fallback and provider selection
 */

import { ProbeAgent } from '../src/agent/ProbeAgent.js';
import chalk from 'chalk';

console.log(chalk.blue('=' .repeat(70)));
console.log(chalk.blue('  CLAUDE CODE INTEGRATION DEMONSTRATION'));
console.log(chalk.blue('=' .repeat(70) + '\n'));

// Custom persona for code review
const codeReviewerPersona = `You are an expert code reviewer specializing in JavaScript/TypeScript.
Your role is to:
- Analyze code quality and patterns
- Identify potential bugs or issues
- Suggest improvements
- Focus on best practices and maintainability`;

async function demonstrateProvider(providerName, persona, question) {
  console.log(chalk.cyan(`\n🔧 Testing ${providerName || 'Auto-Detect'} Provider\n`));
  console.log(chalk.gray('─'.repeat(60)));

  try {
    const agentOptions = {
      customPrompt: persona,
      maxIterations: 3,
      debug: false
    };

    if (providerName) {
      agentOptions.provider = providerName;
    }

    const agent = new ProbeAgent(agentOptions);

    await agent.initialize();

    console.log(chalk.green(`✅ Initialized (provider: ${agent.clientApiProvider || 'auto'})`));

    let toolCalls = [];
    if (agent.events) {
      agent.events.on('toolCall', (event) => {
        if (event.status === 'started') {
          console.log(chalk.yellow(`   Tool: ${event.name}`));
          toolCalls.push(event.name);
        }
      });
    }

    console.log(chalk.magenta(`\n❓ Question: ${question}`));
    const startTime = Date.now();
    const response = await agent.answer(question);
    const duration = ((Date.now() - startTime) / 1000).toFixed(2);

    console.log(chalk.green(`\n✅ Response received (${duration}s)`));
    console.log(chalk.white(response.substring(0, 300) + '...'));

    if (toolCalls.length > 0) {
      console.log(chalk.cyan(`\n📊 Tools used: ${toolCalls.join(', ')}`));
    }

  } catch (error) {
    console.error(chalk.red(`❌ Error: ${error.message}`));
  }
}

async function main() {
  console.log(chalk.yellow('This demo shows Claude Code integration features:\n'));
  console.log(chalk.gray('  • Auto-fallback when no API keys present'));
  console.log(chalk.gray('  • Explicit provider selection'));
  console.log(chalk.gray('  • Custom personas and system prompts'));
  console.log(chalk.gray('  • Tool event extraction\n'));

  // Demo 1: Auto-detect (will use Claude Code if no API keys)
  console.log(chalk.blue('\n1️⃣ AUTO-DETECT MODE'));
  console.log(chalk.gray('─'.repeat(60)));
  await demonstrateProvider(
    null,  // Auto-detect
    codeReviewerPersona,
    'Find error handling patterns in this codebase'
  );

  // Demo 2: Explicit Claude Code
  console.log(chalk.blue('\n\n2️⃣ EXPLICIT CLAUDE CODE PROVIDER'));
  console.log(chalk.gray('─'.repeat(60)));
  await demonstrateProvider(
    'claude-code',
    codeReviewerPersona,
    'Analyze the ProbeAgent class structure'
  );

  console.log(chalk.blue('\n\n' + '=' .repeat(70)));
  console.log(chalk.green('✨ Demonstration Complete!\n'));

  console.log(chalk.cyan('Key Features:'));
  console.log(chalk.gray('  • Single "provider" parameter for all engines'));
  console.log(chalk.gray('  • Auto-detection when provider not specified'));
  console.log(chalk.gray('  • Custom system prompts and personas'));
  console.log(chalk.gray('  • Seamless tool integration'));

  console.log(chalk.cyan('\nUsage Examples:'));
  console.log(chalk.gray('  # Auto-detect (uses API keys or claude command)'));
  console.log(chalk.gray('  const agent = new ProbeAgent({ allowedFolders: ["/path"] })'));
  console.log(chalk.gray('\n  # Explicit Claude Code'));
  console.log(chalk.gray('  const agent = new ProbeAgent({ provider: "claude-code" })'));
  console.log(chalk.gray('\n  # CLI usage'));
  console.log(chalk.gray('  node probe-agent-cli.js --provider claude-code "your question"\n'));
}

main().catch(error => {
  console.error(chalk.red('Fatal error:'), error);
  process.exit(1);
});
