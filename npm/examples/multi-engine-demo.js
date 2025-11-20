#!/usr/bin/env node

/**
 * Demonstration of Multi-Engine Support in ProbeAgent
 * Shows how to use different AI engines with custom prompts and personas
 */

import { ProbeAgent } from './src/agent/ProbeAgent.js';
import chalk from 'chalk';

console.log(chalk.blue('=' .repeat(70)));
console.log(chalk.blue('  MULTI-ENGINE SUPPORT DEMONSTRATION'));
console.log(chalk.blue('=' .repeat(70) + '\n'));

// Custom persona for code review
const codeReviewerPersona = `You are an expert code reviewer specializing in JavaScript/TypeScript.
Your role is to:
- Analyze code quality and patterns
- Identify potential bugs or issues
- Suggest improvements
- Focus on best practices and maintainability

When using code search tools:
- Be thorough but concise
- Provide specific file locations and line numbers
- Explain your findings clearly`;

// Custom persona for documentation
const docWriterPersona = `You are a technical documentation specialist.
Your role is to:
- Create clear, comprehensive documentation
- Explain code functionality in user-friendly terms
- Generate examples and use cases
- Focus on clarity and completeness

When analyzing code:
- Focus on public APIs and interfaces
- Explain the "why" not just the "what"
- Include practical examples`;

async function demonstrateEngine(engineType, persona, question) {
  console.log(chalk.cyan(`\n🔧 Testing ${engineType.toUpperCase()} Engine\n`));
  console.log(chalk.gray('─'.repeat(60)));

  try {
    // Configure ProbeAgent with engine and persona
    const agent = new ProbeAgent({
      engine: engineType,
      customPrompt: persona,
      maxIterations: 3,
      debug: false
    });

    await agent.initialize();

    console.log(chalk.green(`✅ Initialized ${agent.engineType} engine`));

    // Display engine info
    const engine = await agent.getEngine();
    if (engine.getSystemPrompt) {
      const prompt = engine.getSystemPrompt();
      const preview = prompt.substring(0, 150).replace(/\n/g, ' ');
      console.log(chalk.gray(`📋 System prompt preview: ${preview}...`));
    }

    if (engine.getTools) {
      const tools = engine.getTools();
      console.log(chalk.gray(`🔧 Available tools: ${tools.length}`));
    }

    // Track tool calls
    let toolCalls = [];
    agent.events.on('tool:start', (data) => {
      console.log(chalk.yellow(`   Tool: ${data.tool}`));
      toolCalls.push(data.tool);
    });

    // Execute query
    console.log(chalk.magenta(`\n❓ Question: ${question}`));
    const startTime = Date.now();
    const response = await agent.answer(question);
    const duration = ((Date.now() - startTime) / 1000).toFixed(2);

    // Display results
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
  // Check for API key
  const hasKey = process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY;

  if (!hasKey) {
    console.log(chalk.yellow('\n⚠️  No API key found'));
    console.log(chalk.gray('Set ANTHROPIC_API_KEY or OPENAI_API_KEY to test with real APIs'));
    console.log(chalk.gray('Demonstration will show engine initialization only\n'));
  }

  console.log(chalk.blue('\n1️⃣ DEFAULT CONFIGURATION (Vercel Engine, Built-in Prompt)'));
  console.log(chalk.gray('─'.repeat(60)));

  // Test with default Vercel engine and built-in prompt
  await demonstrateEngine(
    'vercel',
    null, // Use default prompt
    'List the JavaScript files in the engines folder'
  );

  console.log(chalk.blue('\n\n2️⃣ VERCEL ENGINE WITH CODE REVIEWER PERSONA'));
  console.log(chalk.gray('─'.repeat(60)));

  // Test Vercel with custom persona
  await demonstrateEngine(
    'vercel',
    codeReviewerPersona,
    'Review the error handling in the ProbeAgent class'
  );

  console.log(chalk.blue('\n\n3️⃣ CLAUDE SDK ENGINE WITH DOCUMENTATION PERSONA'));
  console.log(chalk.gray('─'.repeat(60)));

  // Test Claude SDK with custom persona
  process.env.USE_CLAUDE_SDK = 'true';
  await demonstrateEngine(
    'claude-sdk',
    docWriterPersona,
    'Document the public API methods of ProbeAgent'
  );

  console.log(chalk.blue('\n\n' + '=' .repeat(70)));
  console.log(chalk.green('✨ Multi-Engine Demonstration Complete!\n'));

  // Summary
  console.log(chalk.cyan('Key Features Demonstrated:'));
  console.log(chalk.gray('  • Multiple AI engine support (Vercel, Claude SDK)'));
  console.log(chalk.gray('  • Custom system prompts and personas'));
  console.log(chalk.gray('  • Seamless tool integration across engines'));
  console.log(chalk.gray('  • Backward compatible with existing code'));
  console.log(chalk.gray('  • No direct dependencies (peer deps only)'));

  console.log(chalk.cyan('\nUsage Examples:'));
  console.log(chalk.gray('  # Use Claude SDK engine'));
  console.log(chalk.gray('  USE_CLAUDE_SDK=true node probe-agent-cli.js "your question"'));
  console.log(chalk.gray('\n  # Use specific engine'));
  console.log(chalk.gray('  node probe-agent-cli.js --engine claude-sdk "your question"'));
  console.log(chalk.gray('\n  # In code'));
  console.log(chalk.gray('  const agent = new ProbeAgent({ engine: "claude-sdk", customPrompt: persona })'));
}

// Handle errors gracefully
main().catch((error) => {
  console.error(chalk.red(`\n❌ Fatal error: ${error.message}`));
  if (error.stack) {
    console.error(chalk.gray(error.stack));
  }
  process.exit(1);
});