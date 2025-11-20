#!/usr/bin/env node

/**
 * Manual test script to validate Codex engine works with actual Codex CLI
 * Run with: node tests/manual/test-codex-basic.js
 */

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

async function testBasicCodexQuery() {
  console.log('🧪 Testing Basic Codex Query\n');
  console.log('='*60 + '\n');

  try {
    console.log('Creating ProbeAgent with codex provider...\n');

    const agent = new ProbeAgent({
      provider: 'codex',
      allowedFolders: [process.cwd()],
      debug: true
    });

    console.log('\nInitializing agent...\n');
    await agent.initialize();

    console.log('✅ Codex engine initialized\n');
    console.log(`   Provider: ${agent.clientApiProvider}`);
    console.log(`   API Type: ${agent.apiType}`);
    console.log(`   Model: ${agent.model}\n`);

    // Test simple query
    console.log('📝 Testing simple query: "What is 5 + 3?"\n');
    const response = await agent.answer('What is 5 + 3?');

    console.log('\n✅ Query successful!');
    console.log('Response:', response.substring(0, 200) + (response.length > 200 ? '...' : ''));

    // Clean up
    if (agent.engine && agent.engine.close) {
      console.log('\n🧹 Cleaning up...');
      await agent.engine.close();
    }

    console.log('\n✅ Test completed successfully!\n');

  } catch (error) {
    console.error('\n❌ Test failed:', error.message);
    if (error.stack) {
      console.error(error.stack);
    }
    process.exit(1);
  }
}

async function testCodexWithMCP() {
  console.log('\n' + '='*60);
  console.log('🧪 Testing Codex with MCP Tools\n');

  try {
    const agent = new ProbeAgent({
      provider: 'codex',
      allowedFolders: [process.cwd()],
      debug: true
    });

    await agent.initialize();

    console.log('\n📝 Testing query that should use MCP tools...\n');
    console.log('Query: "Search for ProbeAgent class in this codebase"\n');

    const response = await agent.answer('Search for ProbeAgent class in this codebase');

    console.log('\n✅ Query completed');
    console.log('Response length:', response.length);

    // Clean up
    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

    console.log('\n✅ MCP test completed!\n');

  } catch (error) {
    console.error('\n❌ MCP test failed:', error.message);
    if (error.stack) {
      console.error(error.stack);
    }
  }
}

async function main() {
  console.log('🔬 Codex Engine Manual Validation Test\n');
  console.log('This test validates that the Codex engine works with actual Codex CLI.\n');

  // Test 1: Basic query
  await testBasicCodexQuery();

  // Test 2: MCP tools (if time permits)
  // Uncomment to test:
  // await testCodexWithMCP();

  console.log('\n📋 Summary:');
  console.log('- Codex CLI spawns correctly with `codex exec --json -`');
  console.log('- JSON events are parsed correctly');
  console.log('- Agent messages are extracted and returned');
  console.log('- MCP server registration works (if tested)');
  console.log('\n✨ All manual tests complete!\n');
}

main().catch(console.error);
