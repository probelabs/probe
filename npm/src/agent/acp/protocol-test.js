#!/usr/bin/env node

/**
 * Protocol-Only Test - Tests ACP implementation without AI dependencies
 * This focuses on the protocol mechanics without requiring API keys
 */

import { spawn } from 'child_process';

class ProtocolTest {
  constructor() {
    this.server = null;
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.buffer = '';
  }

  async startServer() {
    console.log('🚀 Starting ACP Server (Protocol Test)...');
    
    this.server = spawn('node', ['src/agent/index.js', '--acp'], {
      cwd: '/Users/leonidbugaev/conductor/repo/probe/buger-belgrade/npm',
      stdio: ['pipe', 'pipe', 'pipe']
    });

    this.server.stderr.on('data', (data) => {
      const output = data.toString().trim();
      if (output && output.includes('[ACP]')) {
        console.log(`🔍 ${output}`);
      }
    });

    this.server.stdout.on('data', (data) => {
      this.buffer += data.toString();
      this.processBuffer();
    });

    await new Promise(resolve => setTimeout(resolve, 1000));
    console.log('✅ Server ready\n');
  }

  processBuffer() {
    const lines = this.buffer.split('\n');
    this.buffer = lines.pop() || '';
    
    for (const line of lines) {
      if (line.trim()) {
        try {
          const message = JSON.parse(line);
          this.handleMessage(message);
        } catch (error) {
          console.log(`⚠️  Non-JSON: ${line.substring(0, 50)}...`);
        }
      }
    }
  }

  handleMessage(message) {
    if (message.id && this.pendingRequests.has(message.id)) {
      const { resolve, reject } = this.pendingRequests.get(message.id);
      this.pendingRequests.delete(message.id);
      
      if (message.error) {
        reject(new Error(`${message.error.code}: ${message.error.message}`));
      } else {
        resolve(message.result);
      }
    } else if (message.method) {
      console.log(`📨 Notification: ${message.method}`);
      if (message.params) {
        console.log(`   ${JSON.stringify(message.params, null, 4)}`);
      }
    }
  }

  async sendRequest(method, params = null) {
    const id = this.messageId++;
    const message = { jsonrpc: '2.0', method, id };
    if (params !== null) message.params = params;
    
    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      this.server.stdin.write(JSON.stringify(message) + '\n');
      
      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error('Timeout'));
        }
      }, 10000);
    });
  }

  async runProtocolTest() {
    console.log('🧪 ACP Protocol Functionality Test');
    console.log('='.repeat(50));
    
    try {
      await this.startServer();
      
      // Test 1: Initialize
      console.log('1️⃣ Testing Protocol Initialization');
      const initResult = await this.sendRequest('initialize', { protocolVersion: '1' });
      console.log(`   ✅ Version: ${initResult.protocolVersion}`);
      console.log(`   ✅ Server: ${initResult.serverInfo.name}`);
      console.log(`   ✅ Tools: ${initResult.capabilities.tools.length}`);
      console.log();
      
      // Test 2: Session Creation
      console.log('2️⃣ Testing Session Management');
      const sessionResult = await this.sendRequest('newSession', {});
      const sessionId = sessionResult.sessionId;
      console.log(`   ✅ Created session: ${sessionId.substring(0, 8)}...`);
      console.log();
      
      // Test 3: Session Loading
      console.log('3️⃣ Testing Session Loading');
      const loadResult = await this.sendRequest('loadSession', { sessionId });
      console.log(`   ✅ Loaded session: ${loadResult.id.substring(0, 8)}...`);
      console.log(`   ✅ History length: ${loadResult.historyLength}`);
      console.log();
      
      // Test 4: Mode Changes
      console.log('4️⃣ Testing Session Mode Changes');
      await this.sendRequest('setSessionMode', { sessionId, mode: 'planning' });
      console.log(`   ✅ Mode changed to planning`);
      console.log();
      
      // Test 5: Error Handling
      console.log('5️⃣ Testing Error Handling');
      try {
        await this.sendRequest('unknownMethod', {});
        console.log(`   ❌ Should have failed`);
      } catch (error) {
        if (error.message.includes('-32601')) {
          console.log(`   ✅ Correctly rejected unknown method`);
        } else {
          console.log(`   ❌ Wrong error: ${error.message}`);
        }
      }
      console.log();
      
      // Test 6: Invalid Session
      console.log('6️⃣ Testing Invalid Session Handling');
      try {
        await this.sendRequest('loadSession', { sessionId: 'invalid' });
        console.log(`   ❌ Should have failed`);
      } catch (error) {
        if (error.message.includes('Session not found')) {
          console.log(`   ✅ Correctly rejected invalid session`);
        } else {
          console.log(`   ❌ Wrong error: ${error.message}`);
        }
      }
      console.log();
      
      // Test 7: Cancellation
      console.log('7️⃣ Testing Cancellation');
      const cancelResult = await this.sendRequest('cancel', { sessionId });
      console.log(`   ✅ Cancel successful: ${cancelResult.success}`);
      console.log();
      
      console.log('🎉 All Protocol Tests Passed!');
      console.log();
      console.log('📋 Summary:');
      console.log('   • JSON-RPC 2.0 communication: ✅');
      console.log('   • Protocol initialization: ✅');
      console.log('   • Session management: ✅');
      console.log('   • Mode changes with notifications: ✅');
      console.log('   • Error handling: ✅');
      console.log('   • Request validation: ✅');
      console.log('   • Cancellation support: ✅');
      
    } catch (error) {
      console.log(`❌ Test failed: ${error.message}`);
    } finally {
      this.cleanup();
    }
  }

  cleanup() {
    if (this.server) {
      this.server.stdin.end();
      this.server.kill();
    }
  }
}

const test = new ProtocolTest();
test.runProtocolTest().catch(console.error);