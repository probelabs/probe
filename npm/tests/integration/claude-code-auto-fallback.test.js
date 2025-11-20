#!/usr/bin/env node

/**
 * Test that ProbeAgent auto-detects and falls back to claude-code provider
 * when no API keys are present but claude command is available
 */

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

async function testAutoFallback() {
  console.log('🧪 Testing Auto-Fallback to Claude Code Provider\n');
  console.log('='*60 + '\n');

  // Temporarily remove all API key env vars
  const savedEnv = {
    ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY,
    ANTHROPIC_AUTH_TOKEN: process.env.ANTHROPIC_AUTH_TOKEN,
    OPENAI_API_KEY: process.env.OPENAI_API_KEY,
    GOOGLE_GENERATIVE_AI_API_KEY: process.env.GOOGLE_GENERATIVE_AI_API_KEY,
    GOOGLE_API_KEY: process.env.GOOGLE_API_KEY,
    AWS_ACCESS_KEY_ID: process.env.AWS_ACCESS_KEY_ID,
    AWS_SECRET_ACCESS_KEY: process.env.AWS_SECRET_ACCESS_KEY,
    AWS_REGION: process.env.AWS_REGION,
    AWS_BEDROCK_API_KEY: process.env.AWS_BEDROCK_API_KEY
  };

  // Remove all API keys
  delete process.env.ANTHROPIC_API_KEY;
  delete process.env.ANTHROPIC_AUTH_TOKEN;
  delete process.env.OPENAI_API_KEY;
  delete process.env.GOOGLE_GENERATIVE_AI_API_KEY;
  delete process.env.GOOGLE_API_KEY;
  delete process.env.AWS_ACCESS_KEY_ID;
  delete process.env.AWS_SECRET_ACCESS_KEY;
  delete process.env.AWS_REGION;
  delete process.env.AWS_BEDROCK_API_KEY;

  try {
    console.log('Creating ProbeAgent with NO API keys...\n');

    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      debug: true
    });

    console.log('\nInitializing agent...\n');
    await agent.initialize();

    // Check if it auto-switched to claude-code
    if (agent.clientApiProvider === 'claude-code' && agent.apiType === 'claude-code') {
      console.log('\n✅ SUCCESS: Auto-detected and switched to claude-code provider!');
      console.log(`   Provider: ${agent.clientApiProvider}`);
      console.log(`   API Type: ${agent.apiType}`);
      console.log(`   Model: ${agent.model}`);

      // Try a simple query
      console.log('\n📝 Testing query: "What is 5 + 3?"\n');
      const response = await agent.answer('What is 5 + 3?');

      if (response && response.length > 0) {
        console.log('✅ Query successful!');
        console.log('Response:', response.substring(0, 100) + (response.length > 100 ? '...' : ''));
      } else {
        console.log('⚠️  Query returned empty response');
      }

      // Clean up
      if (agent.engine && agent.engine.close) {
        await agent.engine.close();
      }

    } else {
      console.log('❌ FAIL: Did not auto-switch to claude-code provider');
      console.log(`   Provider: ${agent.clientApiProvider}`);
      console.log(`   API Type: ${agent.apiType}`);
    }

  } catch (error) {
    if (error.message.includes('claude command not found')) {
      console.log('\n⚠️  Test could not run: claude command not found on system');
      console.log('This is expected if Claude Code is not installed');
      console.log('\nTo install Claude Code:');
      console.log('  https://docs.claude.com/en/docs/claude-code');
    } else {
      console.error('\n❌ Test failed:', error.message);
    }
  } finally {
    // Restore environment variables
    Object.entries(savedEnv).forEach(([key, value]) => {
      if (value !== undefined) {
        process.env[key] = value;
      }
    });

    console.log('\n' + '='*60);
    console.log('🏁 Auto-Fallback Test Complete\n');
  }
}

async function testWithAPIKey() {
  console.log('\n' + '='*60);
  console.log('🧪 Testing Normal Behavior with API Key\n');

  // Set a mock env var
  const hadKey = !!process.env.ANTHROPIC_API_KEY;
  if (!hadKey) {
    process.env.ANTHROPIC_API_KEY = 'test-key';
  }

  try {
    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      debug: false
    });

    await agent.initialize();

    if (agent.apiType !== 'claude-code') {
      console.log('✅ With API key, did NOT auto-switch to claude-code');
      console.log(`   API Type: ${agent.apiType}`);
    } else {
      console.log('⚠️  With API key, unexpectedly switched to claude-code');
    }

  } catch (error) {
    console.log('ℹ️  Expected error with test API key:', error.message.substring(0, 80));
  } finally {
    if (!hadKey) {
      delete process.env.ANTHROPIC_API_KEY;
    }
  }
}

async function main() {
  console.log('🔬 ProbeAgent Auto-Fallback to Claude Code Test\n');
  console.log('This test verifies that ProbeAgent automatically uses claude-code');
  console.log('provider when no API keys are found but claude command is available.\n');

  await testAutoFallback();
  await testWithAPIKey();

  console.log('\n📋 Summary:');
  console.log('- Auto-fallback activates when: No API keys + claude command available');
  console.log('- Falls back to: provider="claude-code"');
  console.log('- User benefit: Zero-config usage in Claude Code environment');
}

main().catch(console.error);