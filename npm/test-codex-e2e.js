#!/usr/bin/env node

/**
 * End-to-end test for Codex integration
 * Run with: node test-codex-e2e.js
 */

import { ProbeAgent } from './src/agent/ProbeAgent.js';

console.log('🧪 Codex Integration E2E Test\n');
console.log('Testing basic query with Codex engine...\n');

async function main() {
  let agent;

  try {
    // Create agent with Codex provider (use default model, not gpt-4o)
    console.log('1️⃣  Creating ProbeAgent with provider: codex (using default model)');
    agent = new ProbeAgent({
      provider: 'codex',
      model: null,  // Don't specify model, let Codex use its default
      allowedFolders: [process.cwd()],
      debug: true
    });

    console.log('\n2️⃣  Initializing agent...');
    await agent.initialize();

    console.log('\n✅ Agent initialized successfully!');
    console.log(`   Provider: ${agent.clientApiProvider}`);
    console.log(`   API Type: ${agent.apiType}`);
    console.log(`   Model: ${agent.model}`);

    // Test simple query
    console.log('\n3️⃣  Testing simple query: "What is 2 + 2?"');
    console.log('   (This should trigger Codex CLI)\n');

    const response = await agent.answer('What is 2 + 2?');

    console.log('\n✅ Query completed!');
    console.log('\n📝 Response:');
    console.log('─'.repeat(60));
    console.log(response);
    console.log('─'.repeat(60));

    // Clean up
    console.log('\n4️⃣  Cleaning up...');
    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

    console.log('\n✅ All tests passed! 🎉\n');
    process.exit(0);

  } catch (error) {
    console.error('\n❌ Test failed:', error.message);
    console.error('\nStack trace:');
    console.error(error.stack);

    // Clean up on error
    if (agent?.engine?.close) {
      try {
        await agent.engine.close();
      } catch (cleanupError) {
        // Ignore cleanup errors
      }
    }

    console.log('\n💡 Common issues:');
    console.log('   - Make sure Codex CLI is installed: https://openai.com/codex');
    console.log('   - Check that you can run: codex --version');
    console.log('   - Ensure you have an active Codex session');

    process.exit(1);
  }
}

main();
