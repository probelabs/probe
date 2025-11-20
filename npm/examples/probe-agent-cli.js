#!/usr/bin/env node

/**
 * CLI for testing ProbeAgent with different engines
 * Usage: node probe-agent-cli.js [--engine vercel|claude-sdk] "your question"
 */

import { ProbeAgent } from './src/agent/ProbeAgent.js';
import chalk from 'chalk';

// Parse command line arguments
const args = process.argv.slice(2);
// Detect engine from environment or default to vercel
let engine = process.env.USE_CLAUDE_CODE === 'true' ? 'claude-code' :
             process.env.USE_CLAUDE_SDK === 'true' ? 'claude-sdk' :
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

// Check for API key (not required for Claude CLI/SDK in Claude Code environment)
if (engine === 'claude-code') {
  // Claude CLI uses the installed claude command
  console.log(colors.debug('Using Claude CLI engine (requires claude command installed)'));
} else if (engine === 'claude-sdk') {
  // Claude SDK can use built-in token in Claude Code environment
  console.log(colors.debug('Using Claude SDK engine (may use Claude Code built-in access)'));
} else if (!process.env.ANTHROPIC_API_KEY && !process.env.OPENAI_API_KEY) {
  console.error(colors.error('⚠️  No API key found. Please set ANTHROPIC_API_KEY or OPENAI_API_KEY'));
  process.exit(1);
}

async function runAgent(engineType) {
  const startTime = Date.now();

  try {
    // Create agent with specified engine
    const agent = new ProbeAgent({
      provider: provider,  // Fixed: use engineType instead of engine
      debug: true, // Enable debug to see tool calls
      allowedFolders: [process.cwd()],
      maxIterations: 5, // Limit iterations for demo
      // Enable streaming to see real-time output
      onStream: (chunk) => {
        // This would show streaming output if implemented
      }
    });

    // Set environment variable for Claude engines if needed
    if (engineType === 'claude-code') {
      process.env.USE_CLAUDE_CODE = 'true';
    } else if (engineType === 'claude-sdk') {
      process.env.USE_CLAUDE_SDK = 'true';
    }

    console.log(colors.debug('Initializing agent...'));
    await agent.initialize();

    // Get engine info
    const actualEngine = await agent.getEngine();
    console.log(colors.engine(`✓ Engine initialized: ${agent.engineType}\n`));

    // Show available tools
    if (actualEngine.getTools) {
      const tools = actualEngine.getTools();
      console.log(colors.tool('Available tools:'));

      if (Array.isArray(tools)) {
        // Claude SDK format
        tools.forEach(tool => {
          console.log(colors.tool(`  • ${tool.name}: ${tool.description?.substring(0, 60)}...`));
        });
      } else if (typeof tools === 'object') {
        // Vercel format
        Object.keys(tools).forEach(name => {
          const tool = tools[name];
          const desc = tool.description || tool.name || name;
          console.log(colors.tool(`  • ${name}: ${typeof desc === 'string' ? desc.substring(0, 60) : name}...`));
        });
      }
      console.log('');
    }

    // Hook into tool execution events
    let toolCallCount = 0;
    agent.events.on('tool:start', (data) => {
      toolCallCount++;
      console.log(colors.tool(`\n🔧 Tool Call #${toolCallCount}: ${data.tool}`));
      console.log(colors.debug(`   Parameters: ${JSON.stringify(data.params, null, 2)}`));
    });

    agent.events.on('tool:complete', (data) => {
      if (data.result) {
        const resultStr = typeof data.result === 'string'
          ? data.result
          : JSON.stringify(data.result, null, 2);

        // Truncate long results
        const preview = resultStr.length > 500
          ? resultStr.substring(0, 500) + '...\n[truncated]'
          : resultStr;

        console.log(colors.result(`   Result: ${preview}`));
      }
    });

    agent.events.on('tool:error', (data) => {
      console.log(colors.error(`   Error: ${data.error}`));
    });

    // Execute the query
    console.log(colors.debug('Executing query...\n'));
    console.log(colors.text('─'.repeat(60)));

    const response = await agent.answer(question);

    console.log(colors.text('─'.repeat(60)));
    console.log(colors.result('\n📝 Final Response:\n'));
    console.log(colors.text(response));

    // Show statistics
    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    const tokenUsage = agent.tokenCounter.getTokenUsage();  // Fixed: use getTokenUsage instead of getTotalUsage

    console.log(colors.debug('\n📊 Statistics:'));
    console.log(colors.debug(`  • Duration: ${duration}s`));
    console.log(colors.debug(`  • Tool calls: ${toolCallCount}`));
    console.log(colors.debug(`  • Request tokens: ${tokenUsage.total.request}`));  // Fixed: use correct property path
    console.log(colors.debug(`  • Response tokens: ${tokenUsage.total.response}`));  // Fixed: use correct property path
    console.log(colors.debug(`  • Total tokens: ${tokenUsage.total.total}`));  // Fixed: use correct property path

    // Cleanup
    if (actualEngine.close) {
      await actualEngine.close();
    }

    // Clean up environment
    delete process.env.USE_CLAUDE_SDK;

  } catch (error) {
    console.error(colors.error(`\n❌ Error: ${error.message}`));

    if (error.message.includes('Claude Agent SDK not installed')) {
      console.log(colors.debug('\n💡 To use Claude SDK engine, install it with:'));
      console.log(colors.debug('   npm install @anthropic-ai/claude-agent-sdk'));
    }

    if (error.stack && process.env.DEBUG) {
      console.error(colors.debug(error.stack));
    }
  }
}

// Main execution
async function main() {
  console.log(colors.debug(`Working directory: ${process.cwd()}`));
  console.log(colors.debug(`Node version: ${process.version}\n`));

  // Show engine configuration
  if (engine === 'claude-sdk') {
    console.log(colors.engine('🤖 Using Claude Agent SDK (if installed)'));
    console.log(colors.debug('   • Native tool execution'));
    console.log(colors.debug('   • 200k context window'));
    console.log(colors.debug('   • Native MCP support\n'));
  } else {
    console.log(colors.engine('🤖 Using Vercel AI SDK (default)'));
    console.log(colors.debug('   • XML-based tool calls'));
    console.log(colors.debug('   • Multiple provider support'));
    console.log(colors.debug('   • 128k context window\n'));
  }

  await runAgent(engine);
}

// Run the CLI
main().catch(error => {
  console.error(colors.error('Fatal error:'), error);
  process.exit(1);
});