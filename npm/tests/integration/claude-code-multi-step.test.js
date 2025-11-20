#!/usr/bin/env node

/**
 * Integration test for multi-step Claude Code responses
 * Tests that we properly handle responses when Claude Code uses internal agents/tools
 */

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

async function testMultiStepQuery() {
  console.log('Testing Multi-Step Claude Code Query (with internal tool use)\n');
  console.log('='*60 + '\n');

  const agent = new ProbeAgent({
    allowedFolders: [process.cwd()],
    debug: false,  // Clean output
    provider: 'claude-code'
  });

  try {
    await agent.initialize();

    // Query that might trigger internal tool use (like Task agent)
    console.log('📝 Query: "Explain how this npm package works"\n');
    console.log('Note: This query may trigger internal tool usage in Claude Code\n');

    let streamedContent = '';
    const result = await agent.answer(
      'Explain how this npm package (ProbeAgent) works - give a brief overview',
      [],
      {
        onStream: (chunk) => {
          streamedContent += chunk;
        }
      }
    );

    console.log('='*60 + '\n');
    console.log('🔍 Response Analysis:\n');

    if (result && result.length > 0) {
      console.log('✅ Got final response from Claude Code');
      console.log('Response length:', result.length, 'characters');
      console.log('\nFirst 300 characters of response:');
      console.log(result.substring(0, 300) + '...\n');

      // Check if response seems complete
      if (result.includes('ProbeAgent') || result.includes('agent') || result.includes('AI')) {
        console.log('✅ Response contains relevant content about ProbeAgent');
      } else {
        console.log('⚠️  Response may not contain expected content');
      }
    } else {
      console.log('❌ FAIL: Empty response received');
      console.log('This indicates the multi-step fix may not be working');
    }

    // Check streaming
    if (streamedContent.length > 0) {
      console.log('\n✅ Streaming also worked');
      console.log('Streamed content length:', streamedContent.length);
    } else {
      console.log('\n⚠️  No streamed content received');
    }

    // Clean up
    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

  } catch (error) {
    console.error('Test failed:', error.message);
  }
}

async function testSimpleQuery() {
  console.log('\n' + '='*60);
  console.log('Testing Simple Claude Code Query (baseline)\n');

  const agent = new ProbeAgent({
    allowedFolders: [process.cwd()],
    debug: false,
    provider: 'claude-code'
  });

  try {
    await agent.initialize();

    console.log('📝 Query: "Say hello"\n');

    const result = await agent.answer('Say hello');

    if (result && result.length > 0) {
      console.log('✅ Simple query works');
      console.log('Response:', result.substring(0, 100));
    } else {
      console.log('❌ Even simple query returned empty');
    }

    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

  } catch (error) {
    console.error('Simple test failed:', error.message);
  }
}

async function main() {
  console.log('🧪 Claude Code Multi-Step Response Integration Test\n');

  // Test simple query first (baseline)
  await testSimpleQuery();

  // Test complex query that may trigger internal tools
  await testMultiStepQuery();

  console.log('\n' + '='*60);
  console.log('🏁 Integration Test Complete\n');
  console.log('Key Points:');
  console.log('- Claude Code may use internal agents/tools (Task, etc.)');
  console.log('- These emit "assistant" type messages');
  console.log('- Without handling "assistant" type, responses appear empty');
  console.log('- The fix adds support for extracting text from "assistant" messages');
}

main().catch(console.error);