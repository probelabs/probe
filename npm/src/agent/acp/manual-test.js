#!/usr/bin/env node

/**
 * Manual ACP Test - Comprehensive testing of the ACP implementation
 * 
 * This test spawns the ACP server and sends real protocol messages to verify
 * the complete functionality including initialization, sessions, and AI interactions.
 */

import { spawn } from 'child_process';
import { randomUUID } from 'crypto';

class ACPManualTest {
  constructor() {
    this.server = null;
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.buffer = '';
    this.sessionId = null;
    this.testResults = [];
  }

  async startServer() {
    console.log('🚀 Starting ACP Server...');
    
    // Start the ACP server with verbose output
    this.server = spawn('node', ['src/agent/index.js', '--acp', '--verbose'], {
      cwd: '/Users/leonidbugaev/conductor/repo/probe/buger-belgrade/npm',
      stdio: ['pipe', 'pipe', 'pipe']
    });

    // Handle server debug output
    this.server.stderr.on('data', (data) => {
      const output = data.toString().trim();
      if (output) {
        // Check if this looks like a JSON-RPC message that was mislabeled
        const lines = output.split('\n');
        for (const line of lines) {
          if (line.trim().startsWith('{"jsonrpc"')) {
            this.buffer += line + '\n';
            this.processBuffer();
          } else if (line.trim()) {
            console.log(`🔍 Server: ${line}`);
          }
        }
      }
    });

    // Handle server responses (JSON-RPC messages on stdout)
    this.server.stdout.on('data', (data) => {
      this.buffer += data.toString();
      this.processBuffer();
    });

    this.server.on('close', (code) => {
      console.log(`📴 Server closed with code ${code}`);
    });

    // Wait for server to start
    await new Promise(resolve => setTimeout(resolve, 2000));
    console.log('✅ Server started\n');
  }

  processBuffer() {
    const lines = this.buffer.split('\n');
    this.buffer = lines.pop() || ''; // Keep incomplete line in buffer
    
    for (const line of lines) {
      if (line.trim()) {
        try {
          const message = JSON.parse(line);
          this.handleMessage(message);
        } catch (error) {
          console.log(`⚠️  Failed to parse: ${line}`);
        }
      }
    }
  }

  handleMessage(message) {
    if (message.id && this.pendingRequests.has(message.id)) {
      // Response to our request
      const { resolve, reject } = this.pendingRequests.get(message.id);
      this.pendingRequests.delete(message.id);
      
      if (message.error) {
        reject(new Error(`RPC Error ${message.error.code}: ${message.error.message}`));
      } else {
        resolve(message.result);
      }
    } else if (message.method) {
      // Notification from server
      console.log(`📨 Notification: ${message.method}`, JSON.stringify(message.params, null, 2));
    }
  }

  async sendRequest(method, params = null, description = '') {
    const id = this.messageId++;
    const message = {
      jsonrpc: '2.0',
      method,
      id
    };
    
    if (params !== null) {
      message.params = params;
    }
    
    console.log(`📤 Sending: ${description || method}`);
    if (params) {
      console.log(`   Params: ${JSON.stringify(params, null, 2)}`);
    }
    
    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      
      // Send to server
      this.server.stdin.write(JSON.stringify(message) + '\n');
      
      // Timeout after 30 seconds
      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error('Request timeout'));
        }
      }, 30000);
    });
  }

  async runTest(testName, testFn) {
    try {
      console.log(`\n🧪 Running: ${testName}`);
      console.log('-'.repeat(50));
      
      const result = await testFn();
      
      console.log(`✅ ${testName} - PASSED`);
      this.testResults.push({ name: testName, status: 'PASSED', result });
      
      return result;
    } catch (error) {
      console.log(`❌ ${testName} - FAILED`);
      console.log(`   Error: ${error.message}`);
      this.testResults.push({ name: testName, status: 'FAILED', error: error.message });
      
      throw error;
    }
  }

  async test1_Initialize() {
    return this.runTest('Initialize Protocol', async () => {
      const result = await this.sendRequest('initialize', {
        protocolVersion: '1'
      }, 'Initialize ACP protocol');
      
      console.log(`📥 Response:`, JSON.stringify(result, null, 2));
      
      // Verify response
      if (result.protocolVersion !== '1') {
        throw new Error('Wrong protocol version in response');
      }
      
      if (!result.serverInfo || result.serverInfo.name !== 'probe-agent-acp') {
        throw new Error('Invalid server info');
      }
      
      if (!result.capabilities || !Array.isArray(result.capabilities.tools)) {
        throw new Error('Invalid capabilities');
      }
      
      console.log(`   ✓ Protocol version: ${result.protocolVersion}`);
      console.log(`   ✓ Server: ${result.serverInfo.name} v${result.serverInfo.version}`);
      console.log(`   ✓ Tools: ${result.capabilities.tools.length} available`);
      console.log(`   ✓ Sessions: ${result.capabilities.sessionManagement}`);
      
      return result;
    });
  }

  async test2_CreateSession() {
    return this.runTest('Create New Session', async () => {
      const result = await this.sendRequest('newSession', {}, 'Create new session');
      
      console.log(`📥 Response:`, JSON.stringify(result, null, 2));
      
      if (!result.sessionId) {
        throw new Error('No session ID returned');
      }
      
      if (result.mode !== 'normal') {
        throw new Error('Wrong default mode');
      }
      
      this.sessionId = result.sessionId;
      
      console.log(`   ✓ Session ID: ${this.sessionId}`);
      console.log(`   ✓ Mode: ${result.mode}`);
      console.log(`   ✓ Created: ${result.createdAt}`);
      
      return result;
    });
  }

  async test3_LoadSession() {
    return this.runTest('Load Existing Session', async () => {
      const result = await this.sendRequest('loadSession', {
        sessionId: this.sessionId
      }, 'Load session');
      
      console.log(`📥 Response:`, JSON.stringify(result, null, 2));
      
      if (result.id !== this.sessionId) {
        throw new Error('Wrong session ID in response');
      }
      
      console.log(`   ✓ Loaded session: ${result.id}`);
      console.log(`   ✓ Mode: ${result.mode}`);
      console.log(`   ✓ History length: ${result.historyLength}`);
      
      return result;
    });
  }

  async test4_SetSessionMode() {
    return this.runTest('Set Session Mode', async () => {
      const result = await this.sendRequest('setSessionMode', {
        sessionId: this.sessionId,
        mode: 'planning'
      }, 'Set session to planning mode');
      
      console.log(`📥 Response:`, JSON.stringify(result, null, 2));
      
      if (!result.success) {
        throw new Error('Mode change failed');
      }
      
      console.log(`   ✓ Mode changed to: planning`);
      
      return result;
    });
  }

  async test5_SimplePrompt() {
    return this.runTest('Simple Prompt (No AI)', async () => {
      // Use a simple question that doesn't require AI API calls
      const result = await this.sendRequest('prompt', {
        sessionId: this.sessionId,
        message: 'List the files in the current directory'
      }, 'Send simple prompt');
      
      console.log(`📥 Response:`, JSON.stringify(result, null, 2));
      
      if (!result.content || !Array.isArray(result.content)) {
        throw new Error('Invalid content format');
      }
      
      if (result.content.length === 0) {
        throw new Error('Empty content array');
      }
      
      const textContent = result.content.find(c => c.type === 'text');
      if (!textContent) {
        throw new Error('No text content found');
      }
      
      console.log(`   ✓ Content blocks: ${result.content.length}`);
      console.log(`   ✓ Response length: ${textContent.text.length} chars`);
      console.log(`   ✓ Session ID: ${result.sessionId}`);
      console.log(`   ✓ Timestamp: ${result.timestamp}`);
      
      return result;
    });
  }

  async test6_Cancel() {
    return this.runTest('Cancel Operation', async () => {
      const result = await this.sendRequest('cancel', {
        sessionId: this.sessionId
      }, 'Cancel session operations');
      
      console.log(`📥 Response:`, JSON.stringify(result, null, 2));
      
      if (!result.success) {
        throw new Error('Cancel failed');
      }
      
      console.log(`   ✓ Cancellation successful`);
      
      return result;
    });
  }

  async test7_ErrorHandling() {
    return this.runTest('Error Handling', async () => {
      try {
        await this.sendRequest('invalidMethod', {}, 'Send invalid method');
        throw new Error('Should have failed with invalid method');
      } catch (error) {
        if (!error.message.includes('RPC Error -32601')) {
          throw new Error(`Wrong error type: ${error.message}`);
        }
        
        console.log(`   ✓ Correctly rejected invalid method`);
        console.log(`   ✓ Error: ${error.message}`);
        
        return { error: error.message };
      }
    });
  }

  async test8_InvalidSession() {
    return this.runTest('Invalid Session Handling', async () => {
      try {
        await this.sendRequest('loadSession', {
          sessionId: 'nonexistent-session-id'
        }, 'Try to load nonexistent session');
        throw new Error('Should have failed with invalid session');
      } catch (error) {
        if (!error.message.includes('Session not found')) {
          throw new Error(`Wrong error type: ${error.message}`);
        }
        
        console.log(`   ✓ Correctly rejected invalid session`);
        console.log(`   ✓ Error: ${error.message}`);
        
        return { error: error.message };
      }
    });
  }

  async runAllTests() {
    console.log('🧪 ACP Manual Test Suite');
    console.log('='.repeat(60));
    console.log('This will test the complete ACP implementation with a real server');
    console.log();
    
    try {
      await this.startServer();
      
      // Run tests in sequence
      await this.test1_Initialize();
      await this.test2_CreateSession();
      await this.test3_LoadSession();
      await this.test4_SetSessionMode();
      await this.test5_SimplePrompt();
      await this.test6_Cancel();
      await this.test7_ErrorHandling();
      await this.test8_InvalidSession();
      
    } catch (error) {
      console.log(`\n💥 Test suite failed: ${error.message}`);
    } finally {
      this.cleanup();
    }
    
    this.showResults();
  }

  cleanup() {
    console.log('\n🧹 Cleaning up...');
    if (this.server) {
      this.server.stdin.end();
      this.server.kill('SIGTERM');
      
      // Force kill if still running after 2 seconds
      setTimeout(() => {
        if (this.server && !this.server.killed) {
          this.server.kill('SIGKILL');
        }
      }, 2000);
    }
  }

  showResults() {
    console.log('\n📊 Test Results Summary');
    console.log('='.repeat(40));
    
    const passed = this.testResults.filter(t => t.status === 'PASSED').length;
    const failed = this.testResults.filter(t => t.status === 'FAILED').length;
    
    for (const result of this.testResults) {
      const icon = result.status === 'PASSED' ? '✅' : '❌';
      console.log(`${icon} ${result.name}`);
      if (result.error) {
        console.log(`     ${result.error}`);
      }
    }
    
    console.log();
    console.log(`Passed: ${passed}`);
    console.log(`Failed: ${failed}`);
    console.log(`Total:  ${this.testResults.length}`);
    
    if (failed === 0) {
      console.log('\n🎉 All tests passed! ACP implementation is working correctly.');
    } else {
      console.log(`\n⚠️  ${failed} test(s) failed. Check the errors above.`);
    }
  }
}

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
  console.log('\n⏹️  Test interrupted');
  process.exit(0);
});

// Run the manual test
const test = new ACPManualTest();
test.runAllTests().catch(console.error);