#!/usr/bin/env node

/**
 * Debug the history issue in ChatSessionManager
 * Test multi-turn conversation handling
 */

import { ChatSessionManager } from './ChatSessionManager.js';
import { JsonChatStorage } from './storage/JsonChatStorage.js';
import { randomUUID } from 'crypto';

// Set test API key
process.env.ANTHROPIC_API_KEY = 'test-key-for-debug';

async function debugHistoryIssue() {
  console.log('🔍 Debugging ChatSessionManager History Issue\n');
  
  try {
    // Create a ChatSessionManager instance
    const storage = new JsonChatStorage({ verbose: false });
    await storage.initialize();
    
    const sessionId = randomUUID();
    const manager = new ChatSessionManager({
      sessionId,
      storage,
      debug: true,
      path: process.cwd()
    });
    
    console.log('1️⃣ Initial state:');
    console.log(`   - ProbeAgent history length: ${manager.agent.history.length}`);
    console.log(`   - Display history length: ${manager.displayHistory.length}`);
    console.log(`   - Session ID: ${sessionId}`);
    
    // Test: Simulate sending first message (without actual API call)
    console.log('\n2️⃣ Simulating first message...');
    
    // Manually add messages to histories to simulate what should happen
    const firstMessage = "What is the purpose of this project?";
    const firstResponse = "This is a code search tool...";
    
    // Add to display history (this happens in chat())
    manager.displayHistory.push({
      role: 'user',
      content: firstMessage,
      timestamp: new Date().toISOString(),
      displayType: 'user',
      visible: true
    });
    
    manager.displayHistory.push({
      role: 'assistant',
      content: firstResponse,
      timestamp: new Date().toISOString(),
      displayType: 'final',
      visible: true
    });
    
    // Check: Does ProbeAgent.history get updated? (THIS IS THE BUG!)
    console.log(`   - ProbeAgent history after first message: ${manager.agent.history.length}`);
    console.log(`   - Display history after first message: ${manager.displayHistory.length}`);
    
    if (manager.agent.history.length === 0) {
      console.log('   ❌ BUG FOUND: ProbeAgent.history is not being updated!');
      console.log('   📝 This means ProbeAgent loses conversation context');
      console.log('   💡 Follow-up messages will be treated as new conversations');
    }
    
    // Test: What should happen for proper multi-turn support
    console.log('\n3️⃣ What should happen for multi-turn support:');
    console.log('   ✅ ProbeAgent.history should be updated after each message');
    console.log('   ✅ ProbeAgent.answer() should receive conversation context');
    console.log('   ✅ Follow-up questions should reference previous conversation');
    
    // Show the fix needed
    console.log('\n4️⃣ Fix needed in ChatSessionManager.chat():');
    console.log('   After calling this.agent.answer(), we should update:');
    console.log('   - this.agent.history.push({ role: "user", content: message })');
    console.log('   - this.agent.history.push({ role: "assistant", content: response })');
    
    console.log('\n5️⃣ Testing history property access...');
    console.log(`   - manager.history (alias): ${manager.history.length} messages`);
    console.log('   - This should mirror agent.history for compatibility');
    
  } catch (error) {
    console.error('Debug test failed:', error.message);
  }
}

debugHistoryIssue().catch(console.error);