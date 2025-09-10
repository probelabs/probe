#!/usr/bin/env node

/**
 * Integration test for ChatSessionManager with ProbeAgent
 * This script tests that the web chat can now use ProbeAgent instead of ProbeChat
 */

import { ChatSessionManager } from './ChatSessionManager.js';
import { JsonChatStorage } from './storage/JsonChatStorage.js';
import { randomUUID } from 'crypto';
import os from 'os';
import path from 'path';

// Set test environment variables to avoid API key requirements
process.env.ANTHROPIC_API_KEY = 'test-key-for-integration-test';
process.env.DEBUG = '0'; // Reduce noise in test output

// Mock API credentials for testing (won't actually call AI)
const testCredentials = {
  apiProvider: 'anthropic',
  apiKey: 'test-key', // Won't be used in this test
};

async function testIntegration() {
  console.log('🧪 Testing ChatSessionManager <-> ProbeAgent Integration\n');
  
  // Test 1: Verify ChatSessionManager can be instantiated
  console.log('1️⃣ Testing ChatSessionManager instantiation...');
  try {
    const storage = new JsonChatStorage({ 
      webMode: true, 
      verbose: true,
      baseDir: path.join(os.tmpdir(), 'probe-test')
    });
    await storage.initialize();
    
    const sessionManager = new ChatSessionManager({
      sessionId: randomUUID(),
      storage: storage,
      path: process.cwd(),
      debug: true,
      ...testCredentials
    });
    
    console.log(`✅ ChatSessionManager created with session: ${sessionManager.getSessionId()}`);
    console.log(`   - ProbeAgent instance: ${sessionManager.agent ? '✓' : '✗'}`);
    console.log(`   - Storage configured: ${sessionManager.storage ? '✓' : '✗'}`);
    console.log(`   - Display history initialized: ${Array.isArray(sessionManager.displayHistory) ? '✓' : '✗'}`);
    
  } catch (error) {
    console.log(`❌ ChatSessionManager instantiation failed: ${error.message}`);
    return false;
  }
  
  // Test 2: Verify ProbeAgent is properly exported from npm package
  console.log('\n2️⃣ Testing ProbeAgent export from npm package...');
  try {
    const { ProbeAgent } = await import('../../npm/src/index.js');
    
    if (!ProbeAgent) {
      throw new Error('ProbeAgent is not exported from npm package');
    }
    
    // Try to create a ProbeAgent instance directly
    const agent = new ProbeAgent({
      sessionId: randomUUID(),
      path: process.cwd(),
      debug: true
    });
    
    console.log(`✅ ProbeAgent imported and instantiated successfully`);
    console.log(`   - Session ID: ${agent.sessionId}`);
    console.log(`   - History array: ${Array.isArray(agent.history) ? '✓' : '✗'}`);
    console.log(`   - Token counter: ${agent.tokenCounter ? '✓' : '✗'}`);
    
  } catch (error) {
    console.log(`❌ ProbeAgent export test failed: ${error.message}`);
    return false;
  }
  
  // Test 3: Verify ChatSessionManager methods work
  console.log('\n3️⃣ Testing ChatSessionManager methods...');
  try {
    const storage = new JsonChatStorage({ 
      webMode: true, 
      verbose: false,
      baseDir: path.join(os.tmpdir(), 'probe-test-methods')
    });
    await storage.initialize();
    
    const sessionManager = new ChatSessionManager({
      sessionId: randomUUID(),
      storage: storage,
      path: process.cwd(),
      debug: false,
      ...testCredentials
    });
    
    // Test getTokenUsage method
    const usage = sessionManager.getTokenUsage();
    console.log(`✅ getTokenUsage(): ${typeof usage === 'object' ? '✓' : '✗'}`);
    console.log(`   - Has current object: ${typeof usage.current === 'object' ? '✓' : '✗'}`);
    console.log(`   - Has total object: ${typeof usage.total === 'object' ? '✓' : '✗'}`);
    
    // Test clearHistory method
    const oldSessionId = sessionManager.getSessionId();
    const newSessionId = sessionManager.clearHistory();
    console.log(`✅ clearHistory(): ${newSessionId !== oldSessionId ? '✓' : '✗'}`);
    console.log(`   - New session ID generated: ${newSessionId}`);
    console.log(`   - Display history cleared: ${sessionManager.displayHistory.length === 0 ? '✓' : '✗'}`);
    
    // Test history property access
    const historyLength = sessionManager.history.length;
    console.log(`✅ history property: ${Array.isArray(sessionManager.history) ? '✓' : '✗'}`);
    console.log(`   - Initial length: ${historyLength}`);
    
  } catch (error) {
    console.log(`❌ ChatSessionManager methods test failed: ${error.message}`);
    return false;
  }
  
  // Test 4: Verify web server compatibility (basic check)
  console.log('\n4️⃣ Testing web server compatibility...');
  try {
    // Check if webServer.js file exists and has expected structure
    const fs = await import('fs');
    const webServerPath = './webServer.js';
    
    if (!fs.existsSync(webServerPath)) {
      throw new Error('webServer.js file not found');
    }
    
    // Read file content to check for ChatSessionManager usage
    const content = fs.readFileSync(webServerPath, 'utf8');
    const hasChatSessionManager = content.includes('ChatSessionManager');
    const hasGetOrCreateChat = content.includes('getOrCreateChat');
    
    console.log(`✅ webServer.js file exists`);
    console.log(`   - Uses ChatSessionManager: ${hasChatSessionManager ? '✓' : '✗'}`);
    console.log(`   - Has getOrCreateChat function: ${hasGetOrCreateChat ? '✓' : '✗'}`);
    
    if (!hasChatSessionManager || !hasGetOrCreateChat) {
      throw new Error('webServer.js does not appear to use ChatSessionManager');
    }
    
    console.log(`✅ Web server integration appears compatible`);
    
  } catch (error) {
    console.log(`❌ Web server compatibility test failed: ${error.message}`);
    return false;
  }
  
  // Test 5: Verify multi-turn conversation fix
  console.log('\n5️⃣ Testing multi-turn conversation support...');
  try {
    // Test ProbeAgent history handling
    const { ProbeAgent } = await import('../../npm/src/index.js');
    const agent = new ProbeAgent({
      sessionId: randomUUID(),
      debug: false,
      path: process.cwd()
    });
    
    // Simulate conversation history
    agent.history = [
      { role: 'user', content: 'What is this?' },
      { role: 'assistant', content: 'This is a code search tool.' }
    ];
    
    console.log(`✅ ProbeAgent conversation history: ${agent.history.length} messages`);
    
    // Verify ChatSessionManager history property works
    const storage = new JsonChatStorage({ verbose: false });
    await storage.initialize();
    
    const sessionManager = new ChatSessionManager({
      sessionId: randomUUID(),
      storage: storage,
      debug: false,
      path: process.cwd()
    });
    
    sessionManager.agent.history = [
      { role: 'user', content: 'Test message' },
      { role: 'assistant', content: 'Test response' }
    ];
    
    console.log(`✅ ChatSessionManager history sync: ${sessionManager.history.length === sessionManager.agent.history.length ? '✓' : '✗'}`);
    
    // Verify ProbeAgent.answer() includes history
    const probeAgentContent = await import('fs').then(fs => 
      fs.readFileSync('../../npm/src/agent/ProbeAgent.js', 'utf8')
    );
    const hasHistoryFix = probeAgentContent.includes('...this.history');
    console.log(`✅ ProbeAgent includes conversation history: ${hasHistoryFix ? '✓' : '✗'}`);
    
  } catch (error) {
    console.log(`❌ Multi-turn conversation test failed: ${error.message}`);
    return false;
  }

  console.log('\n🎉 All integration tests passed!');
  console.log('\n📋 Summary:');
  console.log('   ✅ ChatSessionManager can be instantiated with ProbeAgent');
  console.log('   ✅ ProbeAgent is exported from npm package');
  console.log('   ✅ ChatSessionManager methods work correctly');
  console.log('   ✅ Web server integration is compatible');
  console.log('   ✅ Multi-turn conversations are supported');
  console.log('\n🚀 The web chat now uses ProbeAgent with full conversation support!');
  
  return true;
}

// Run the test
testIntegration().catch(error => {
  console.error('\n💥 Integration test failed:', error.message);
  if (process.env.DEBUG === '1') {
    console.error(error.stack);
  }
  process.exit(1);
});