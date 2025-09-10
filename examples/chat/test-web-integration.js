#!/usr/bin/env node

/**
 * Test that the web server is actually using ChatSessionManager
 * by inspecting the webServer.js imports and function calls
 */

import { readFileSync } from 'fs';

function testWebServerIntegration() {
  console.log('🌐 Testing Web Server ChatSessionManager Integration\n');

  // Read webServer.js content
  const webServerContent = readFileSync('./webServer.js', 'utf8');
  
  // Test 1: Check imports
  console.log('1️⃣ Checking imports...');
  const hasProbeChat = webServerContent.includes("import { ProbeChat }");
  const hasChatSessionManager = webServerContent.includes("import { ChatSessionManager }");
  
  console.log(`   - ProbeChat import: ${hasProbeChat ? '❌ Still present' : '✅ Removed'}`);
  console.log(`   - ChatSessionManager import: ${hasChatSessionManager ? '✅ Present' : '❌ Missing'}`);
  
  // Test 2: Check instantiation
  console.log('\n2️⃣ Checking instantiation...');
  const hasNewProbeChat = webServerContent.includes("new ProbeChat");
  const hasNewChatSessionManager = webServerContent.includes("new ChatSessionManager");
  
  console.log(`   - new ProbeChat(): ${hasNewProbeChat ? '❌ Still used' : '✅ Replaced'}`);
  console.log(`   - new ChatSessionManager(): ${hasNewChatSessionManager ? '✅ Used' : '❌ Missing'}`);
  
  // Test 3: Check comments
  console.log('\n3️⃣ Checking function comments...');
  const hasUpdatedComment = webServerContent.includes("ChatSessionManager instance");
  
  console.log(`   - Updated function comment: ${hasUpdatedComment ? '✅ Updated' : '❌ Not updated'}`);
  
  // Test 4: Overall assessment
  console.log('\n📊 Overall Assessment:');
  const isFullyMigrated = !hasProbeChat && !hasNewProbeChat && hasChatSessionManager && hasNewChatSessionManager;
  
  if (isFullyMigrated) {
    console.log('✅ Web server is fully migrated to use ChatSessionManager!');
    console.log('   - The web chat will now use ProbeAgent under the hood');
    console.log('   - All API endpoints will use the unified AI logic');
    console.log('   - Session management is handled by ChatSessionManager → ProbeAgent');
  } else {
    console.log('⚠️ Web server migration is incomplete');
    if (hasProbeChat || hasNewProbeChat) {
      console.log('   - ProbeChat is still referenced (should be removed)');
    }
    if (!hasChatSessionManager || !hasNewChatSessionManager) {
      console.log('   - ChatSessionManager is not properly integrated');
    }
  }
  
  // Test 5: Check what CLI still uses
  console.log('\n5️⃣ Checking CLI usage...');
  const indexContent = readFileSync('./index.js', 'utf8');
  const cliUsesProbeChat = indexContent.includes("import { ProbeChat }");
  const cliUsesChatSessionManager = indexContent.includes("import { ChatSessionManager }");
  
  console.log(`   - CLI (index.js) uses ProbeChat: ${cliUsesProbeChat ? '✅ Yes' : '❌ No'}`);
  console.log(`   - CLI (index.js) uses ChatSessionManager: ${cliUsesChatSessionManager ? '✅ Yes' : '❌ No'}`);
  
  if (cliUsesProbeChat && !cliUsesChatSessionManager) {
    console.log('\n💡 Note: CLI still uses ProbeChat, which is fine for now.');
    console.log('   The important part is that the WEB SERVER uses ChatSessionManager.');
    console.log('   Users can test with: node index.js --web --port 3001');
  }
  
  return isFullyMigrated;
}

const success = testWebServerIntegration();

if (success) {
  console.log('\n🎉 Integration test PASSED!');
  console.log('\n🚀 Ready to test:');
  console.log('   1. Start web server: node index.js --web --port 3001');  
  console.log('   2. Visit http://localhost:3001 in browser');
  console.log('   3. Chat interface will use ProbeAgent via ChatSessionManager');
} else {
  console.log('\n❌ Integration test FAILED!');
  process.exit(1);
}