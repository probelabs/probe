#!/usr/bin/env node

/**
 * Simple test runner for ACP implementation
 * Tests basic functionality without requiring a full test framework
 */

import { ACPServer } from './server.js';
import { ACPConnection } from './connection.js';
import { validateMessage, createMessage, createResponse, ACP_PROTOCOL_VERSION } from './types.js';
import { EventEmitter } from 'events';

class TestRunner {
  constructor() {
    this.tests = [];
    this.passed = 0;
    this.failed = 0;
  }

  test(name, fn) {
    this.tests.push({ name, fn });
  }

  assert(condition, message) {
    if (!condition) {
      throw new Error(message);
    }
  }

  assertEqual(actual, expected, message = '') {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(`${message}\nExpected: ${JSON.stringify(expected)}\nActual: ${JSON.stringify(actual)}`);
    }
  }

  async run() {
    console.log('🧪 Running ACP Tests');
    console.log('=' .repeat(40));

    for (const { name, fn } of this.tests) {
      try {
        await fn();
        console.log(`✅ ${name}`);
        this.passed++;
      } catch (error) {
        console.log(`❌ ${name}`);
        console.log(`   Error: ${error.message}`);
        this.failed++;
      }
    }

    console.log('\n📊 Test Results');
    console.log('-'.repeat(20));
    console.log(`Passed: ${this.passed}`);
    console.log(`Failed: ${this.failed}`);
    console.log(`Total:  ${this.tests.length}`);

    if (this.failed > 0) {
      process.exit(1);
    } else {
      console.log('\n🎉 All tests passed!');
    }
  }
}

const runner = new TestRunner();

// Test types and utilities
runner.test('Message validation', () => {
  // Valid messages
  runner.assert(validateMessage({ jsonrpc: '2.0', method: 'test' }).valid, 'Valid notification should pass');
  runner.assert(validateMessage({ jsonrpc: '2.0', method: 'test', id: 1 }).valid, 'Valid request should pass');
  runner.assert(validateMessage({ jsonrpc: '2.0', id: 1, result: {} }).valid, 'Valid response should pass');

  // Invalid messages
  runner.assert(!validateMessage(null).valid, 'Null should fail');
  runner.assert(!validateMessage({ jsonrpc: '1.0', method: 'test' }).valid, 'Wrong version should fail');
  runner.assert(!validateMessage({ jsonrpc: '2.0', id: 1 }).valid, 'Response without result/error should fail');
});

runner.test('Message creation', () => {
  const message = createMessage('test', { param: 'value' }, 123);
  runner.assertEqual(message, {
    jsonrpc: '2.0',
    method: 'test',
    params: { param: 'value' },
    id: 123
  }, 'Message creation should work correctly');

  const response = createResponse(456, { success: true });
  runner.assertEqual(response, {
    jsonrpc: '2.0',
    id: 456,
    result: { success: true }
  }, 'Response creation should work correctly');
});

// Test server capabilities
runner.test('Server capabilities', () => {
  const server = new ACPServer();
  const capabilities = server.getCapabilities();

  runner.assert(Array.isArray(capabilities.tools), 'Should have tools array');
  runner.assert(capabilities.tools.length === 3, 'Should have 3 tools');
  runner.assert(capabilities.sessionManagement === true, 'Should support sessions');
  runner.assert(capabilities.streaming === true, 'Should support streaming');

  const toolNames = capabilities.tools.map(t => t.name);
  runner.assert(toolNames.includes('search'), 'Should have search tool');
  runner.assert(toolNames.includes('query'), 'Should have query tool');
  runner.assert(toolNames.includes('extract'), 'Should have extract tool');
});

// Test server initialization
runner.test('Server initialization', async () => {
  const server = new ACPServer({ debug: false });

  // Test valid initialization
  const result = await server.handleInitialize({
    protocolVersion: ACP_PROTOCOL_VERSION
  });

  runner.assertEqual(result.protocolVersion, ACP_PROTOCOL_VERSION, 'Should return correct version');
  runner.assert(result.serverInfo.name === 'probe-agent-acp', 'Should have correct server name');
  runner.assert(server.initialized === true, 'Server should be initialized');

  // Test invalid protocol version
  try {
    await server.handleInitialize({ protocolVersion: '2.0' });
    runner.assert(false, 'Should reject invalid protocol version');
  } catch (error) {
    runner.assert(error.message.includes('Unsupported protocol version'), 'Should have version error');
  }
});

// Test session management
runner.test('Session management', async () => {
  const server = new ACPServer({ debug: false });

  // Create session
  const createResult = await server.handleNewSession({});
  runner.assert(createResult.sessionId, 'Should return session ID');
  runner.assert(createResult.mode === 'normal', 'Should default to normal mode');
  runner.assert(server.sessions.has(createResult.sessionId), 'Should store session');

  // Load session
  const loadResult = await server.handleLoadSession({ 
    sessionId: createResult.sessionId 
  });
  runner.assertEqual(loadResult.id, createResult.sessionId, 'Should load correct session');

  // Set session mode
  const modeResult = await server.handleSetSessionMode({
    sessionId: createResult.sessionId,
    mode: 'planning'
  });
  runner.assert(modeResult.success === true, 'Should set mode successfully');
  
  const session = server.sessions.get(createResult.sessionId);
  runner.assertEqual(session.mode, 'planning', 'Should update session mode');
});

// Test connection (basic functionality)
runner.test('Connection basics', () => {
  class MockStream extends EventEmitter {
    constructor() {
      super();
      this.encoding = null;
      this.writtenData = [];
    }
    setEncoding(enc) { this.encoding = enc; }
    write(data) { this.writtenData.push(data); return true; }
  }

  const input = new MockStream();
  const output = new MockStream();
  const connection = new ACPConnection(input, output);

  connection.start();
  runner.assert(connection.isConnected === true, 'Should be connected');

  // Test message sending
  connection.sendNotification('test', { data: 'value' });
  runner.assert(output.writtenData.length === 1, 'Should send notification');

  const sent = JSON.parse(output.writtenData[0]);
  runner.assertEqual(sent.method, 'test', 'Should send correct method');
  runner.assertEqual(sent.params, { data: 'value' }, 'Should send correct params');
  runner.assert(!sent.id, 'Notification should not have ID');

  connection.close();
});

// Test error handling
runner.test('Error handling', async () => {
  const server = new ACPServer({ debug: false });

  // Test missing session
  try {
    await server.handleLoadSession({ sessionId: 'nonexistent' });
    runner.assert(false, 'Should fail for nonexistent session');
  } catch (error) {
    runner.assert(error.message.includes('Session not found'), 'Should have session error');
  }

  // Test missing parameters
  try {
    await server.handlePrompt({});
    runner.assert(false, 'Should require sessionId and message');
  } catch (error) {
    runner.assert(error.message.includes('Invalid params'), 'Should have params error');
  }
});

// Run all tests
runner.run().catch(console.error);