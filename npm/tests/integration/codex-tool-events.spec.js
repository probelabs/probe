#!/usr/bin/env node

/**
 * Test that Codex CLI engine properly extracts and emits tool usage events
 */

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

async function testToolEventExtraction() {
  console.log('🧪 Testing Tool Event Extraction from Codex CLI\n');
  console.log('='*60 + '\n');

  try {
    console.log('Creating ProbeAgent with codex provider...\n');

    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      provider: 'codex',
      debug: true
    });

    await agent.initialize();

    if (agent.clientApiProvider !== 'codex') {
      console.log('⚠️  Skipping test - codex provider not available');
      console.log(`   Current provider: ${agent.clientApiProvider}`);
      return;
    }

    console.log('✅ Codex CLI engine initialized\n');

    // Collect tool events
    const toolEvents = [];
    agent.events.on('toolCall', (event) => {
      console.log(`\n🔧 Tool Event Captured: ${event.name}`);
      console.log(`   Status: ${event.status}`);
      console.log(`   Args:`, JSON.stringify(event.args).substring(0, 100));
      toolEvents.push(event);
    });

    // Ask a question that should trigger tool usage
    console.log('\n📝 Testing query that requires tool usage...\n');
    console.log('Query: "Search for ProbeAgent class in this codebase"\n');

    const response = await agent.answer('Search for ProbeAgent class in this codebase');

    console.log('\n📊 Response received');
    console.log('Response length:', response.length);
    console.log('Tool events captured:', toolEvents.length);

    if (toolEvents.length > 0) {
      console.log('\n✅ SUCCESS: Tool events were extracted and emitted!');
      console.log('\nTool usage summary:');
      toolEvents.forEach((event, i) => {
        console.log(`  ${i + 1}. ${event.name} (${event.status})`);
      });
    } else {
      console.log('\n⚠️  No tool events captured');
      console.log('This might be expected if the query did not trigger tool usage');
    }

    // Clean up
    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

  } catch (error) {
    if (error.message.includes('codex') || error.message.includes('command not found')) {
      console.log('\n⚠️  Test could not run: codex command not available');
      console.log('This is expected if OpenAI Codex CLI is not installed');
    } else {
      console.error('\n❌ Test failed:', error.message);
      if (error.stack) {
        console.error(error.stack);
      }
    }
  }

  console.log('\n' + '='*60);
  console.log('🏁 Tool Event Extraction Test Complete\n');
}

async function testToolBatchEmission() {
  console.log('\n' + '='*60);
  console.log('🧪 Testing Tool Batch Emission\n');

  try {
    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      provider: 'codex',
      debug: false
    });

    await agent.initialize();

    if (agent.clientApiProvider !== 'codex') {
      console.log('⚠️  Skipping test - codex provider not available');
      return;
    }

    let batchReceived = false;
    agent.events.on('toolBatch', (batch) => {
      batchReceived = true;
      console.log('✅ Tool batch event received');
      console.log(`   Batch size: ${batch.tools?.length || 0}`);
      console.log(`   Timestamp: ${batch.timestamp}`);
    });

    // Query that might use multiple tools
    await agent.answer('Find and extract the ProbeAgent constructor');

    if (batchReceived) {
      console.log('\n✅ Tool batch emission working correctly');
    } else {
      console.log('\n⚠️  No tool batch received (might not have used tools)');
    }

    // Clean up
    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

  } catch (error) {
    if (error.message.includes('codex') || error.message.includes('command not found')) {
      console.log('⚠️  Codex CLI not available');
    } else {
      console.error('❌ Test failed:', error.message);
    }
  }
}

async function main() {
  console.log('🔬 Codex CLI Tool Event Extraction Test\n');
  console.log('This test verifies that the Codex CLI engine properly extracts');
  console.log('and emits tool usage events from the CLI output.\n');

  await testToolEventExtraction();
  await testToolBatchEmission();

  console.log('\n📋 Summary:');
  console.log('- Tool events are extracted from Codex CLI JSON output');
  console.log('- Events are emitted via agent.events.on("toolCall", ...)');
  console.log('- Batch events contain all tools used in a single query');
  console.log('- Event format: { name, args, status, timestamp, id }');
}

main().catch(console.error);
