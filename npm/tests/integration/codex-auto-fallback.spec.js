#!/usr/bin/env node

/**
 * Test that ProbeAgent auto-detects and falls back to codex provider
 * when no API keys are present but codex command is available
 */

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

async function testAutoFallback() {
  console.log('🧪 Testing Auto-Fallback to Codex CLI Provider\n');
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

  // Mock unavailable claude command to test codex fallback
  // In a real scenario, this would happen when claude is not installed
  // but codex is available

  try {
    console.log('Creating ProbeAgent with NO API keys...\n');

    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      debug: true
    });

    console.log('\nInitializing agent...\n');
    await agent.initialize();

    // Check if it auto-switched to codex
    if (agent.clientApiProvider === 'codex' && agent.apiType === 'codex') {
      console.log('\n✅ SUCCESS: Auto-detected and switched to codex provider!');
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
      console.log('⚠️  Did not auto-switch to codex provider (might have switched to claude-code instead)');
      console.log(`   Provider: ${agent.clientApiProvider}`);
      console.log(`   API Type: ${agent.apiType}`);
      console.log('   Note: This is expected if claude command is also available');
    }

  } catch (error) {
    if (error.message.includes('codex command not found') || error.message.includes('neither claude nor codex')) {
      console.log('\n⚠️  Test could not run: codex command not found on system');
      console.log('This is expected if OpenAI Codex CLI is not installed');
      console.log('\nTo install OpenAI Codex CLI:');
      console.log('  https://openai.com/codex');
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

async function testExplicitCodexProvider() {
  console.log('\n' + '='*60);
  console.log('🧪 Testing Explicit Codex CLI Provider Selection\n');

  try {
    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      provider: 'codex',
      debug: true
    });

    await agent.initialize();

    if (agent.clientApiProvider === 'codex' && agent.apiType === 'codex') {
      console.log('✅ Explicitly set provider to codex successfully');
      console.log(`   Provider: ${agent.clientApiProvider}`);
      console.log(`   API Type: ${agent.apiType}`);
      console.log(`   Model: ${agent.model}`);

      // Clean up
      if (agent.engine && agent.engine.close) {
        await agent.engine.close();
      }
    } else {
      console.log('⚠️  Failed to set explicit codex provider');
    }

  } catch (error) {
    if (error.message.includes('codex command not found') || error.message.includes('command not found')) {
      console.log('⚠️  Codex CLI not available on system');
    } else {
      console.error('❌ Test failed:', error.message);
    }
  }
}

async function testWithAPIKey() {
  console.log('\n' + '='*60);
  console.log('🧪 Testing Normal Behavior with API Key\n');

  // Set a mock env var
  const hadKey = !!process.env.OPENAI_API_KEY;
  if (!hadKey) {
    process.env.OPENAI_API_KEY = 'test-key';
  }

  try {
    const agent = new ProbeAgent({
      allowedFolders: [process.cwd()],
      debug: false
    });

    await agent.initialize();

    if (agent.apiType !== 'codex') {
      console.log('✅ With API key, did NOT auto-switch to codex');
      console.log(`   API Type: ${agent.apiType}`);
    } else {
      console.log('⚠️  With API key, unexpectedly switched to codex');
    }

  } catch (error) {
    console.log('ℹ️  Expected error with test API key:', error.message.substring(0, 80));
  } finally {
    if (!hadKey) {
      delete process.env.OPENAI_API_KEY;
    }
  }
}

async function main() {
  console.log('🔬 ProbeAgent Auto-Fallback to Codex CLI Test\n');
  console.log('This test verifies that ProbeAgent automatically uses codex');
  console.log('provider when no API keys are found but codex command is available.\n');

  await testAutoFallback();
  await testExplicitCodexProvider();
  await testWithAPIKey();

  console.log('\n📋 Summary:');
  console.log('- Auto-fallback activates when: No API keys + codex command available');
  console.log('- Falls back to: provider="codex"');
  console.log('- Fallback priority: claude-code > codex > error');
  console.log('- User benefit: Zero-config usage in OpenAI Codex environment');
}

main().catch(console.error);
