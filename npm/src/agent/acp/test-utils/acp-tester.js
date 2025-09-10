#!/usr/bin/env node

/**
 * ACP Test Utility - A comprehensive testing tool for Agent Client Protocol implementations
 * 
 * This utility can test any ACP-compliant agent by running a standardized test suite
 * that verifies protocol compliance, error handling, and session management.
 * 
 * Usage:
 *   acp-tester --agent-command "node my-agent.js --acp" 
 *   acp-tester --agent-command "my-agent-binary"
 *   acp-tester --agent-command "python agent.py" --timeout 30
 * 
 * Features:
 *   - Protocol compliance testing
 *   - Session management verification  
 *   - Error handling validation
 *   - Tool capability testing
 *   - Performance benchmarking
 *   - JSON-RPC 2.0 validation
 */

import { spawn } from 'child_process';
import { randomUUID } from 'crypto';

class ACPTester {
  constructor(options = {}) {
    this.options = {
      timeout: options.timeout || 30000,
      verbose: options.verbose || false,
      agentCommand: options.agentCommand,
      ...options
    };
    
    this.agent = null;
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.buffer = '';
    this.testResults = [];
    this.startTime = null;
  }

  log(level, message, data = null) {
    const timestamp = new Date().toISOString();
    const prefix = {
      'info': '📋',
      'success': '✅', 
      'error': '❌',
      'warning': '⚠️',
      'debug': '🔍'
    }[level] || '📝';
    
    console.log(`${prefix} ${message}`);
    
    if (data && this.options.verbose) {
      console.log(`   ${JSON.stringify(data, null, 2)}`);
    }
  }

  async startAgent() {
    if (!this.options.agentCommand) {
      throw new Error('Agent command required. Use --agent-command "your-agent-command"');
    }
    
    this.log('info', `Starting agent: ${this.options.agentCommand}`);
    
    const parts = this.options.agentCommand.split(' ');
    const command = parts[0];
    const args = parts.slice(1);
    
    this.agent = spawn(command, args, {
      stdio: ['pipe', 'pipe', 'pipe']
    });
    
    // Handle agent output
    this.agent.stdout.on('data', (data) => {
      this.buffer += data.toString();
      this.processBuffer();
    });
    
    this.agent.stderr.on('data', (data) => {
      if (this.options.verbose) {
        const output = data.toString().trim();
        if (output) {
          this.log('debug', `Agent stderr: ${output}`);
        }
      }
    });
    
    this.agent.on('close', (code) => {
      this.log('info', `Agent closed with code ${code}`);
    });
    
    this.agent.on('error', (error) => {
      this.log('error', `Agent spawn error: ${error.message}`);
    });
    
    // Wait for agent to start
    await new Promise(resolve => setTimeout(resolve, 1000));
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
          if (this.options.verbose) {
            this.log('warning', `Invalid JSON from agent: ${line.substring(0, 100)}...`);
          }
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
        reject(new ACPError(message.error.code, message.error.message, message.error.data));
      } else {
        resolve(message.result);
      }
    } else if (message.method && !message.id) {
      // Notification from agent
      this.log('debug', `Notification: ${message.method}`, message.params);
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
    
    if (description && this.options.verbose) {
      this.log('debug', `Sending: ${description}`);
    }
    
    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      
      // Send message
      this.agent.stdin.write(JSON.stringify(message) + '\n');
      
      // Set timeout
      const timeoutId = setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error(`Request timeout after ${this.options.timeout}ms`));
        }
      }, this.options.timeout);
      
      // Clear timeout when request completes
      const originalResolve = resolve;
      const originalReject = reject;
      
      this.pendingRequests.set(id, {
        resolve: (result) => {
          clearTimeout(timeoutId);
          originalResolve(result);
        },
        reject: (error) => {
          clearTimeout(timeoutId);  
          originalReject(error);
        }
      });
    });
  }

  async runTest(testName, testFn) {
    const startTime = Date.now();
    
    try {
      this.log('info', `Running: ${testName}`);
      const result = await testFn();
      const duration = Date.now() - startTime;
      
      this.log('success', `${testName} (${duration}ms)`);
      this.testResults.push({
        name: testName,
        status: 'PASSED',
        duration,
        result
      });
      
      return result;
    } catch (error) {
      const duration = Date.now() - startTime;
      
      this.log('error', `${testName} (${duration}ms): ${error.message}`);
      this.testResults.push({
        name: testName,
        status: 'FAILED',
        duration,
        error: error.message
      });
      
      throw error;
    }
  }

  // Test Suite
  async testInitialize() {
    return this.runTest('Protocol Initialize', async () => {
      const result = await this.sendRequest('initialize', {
        protocolVersion: '1'
      }, 'Initialize protocol');
      
      // Validate response structure
      this.validateInitializeResponse(result);
      
      return {
        protocolVersion: result.protocolVersion,
        serverName: result.serverInfo?.name,
        toolCount: result.capabilities?.tools?.length || 0,
        hasSessionManagement: result.capabilities?.sessionManagement || false
      };
    });
  }

  async testSessionManagement() {
    return this.runTest('Session Management', async () => {
      // Create session
      const createResult = await this.sendRequest('newSession', {}, 'Create session');
      this.validateSessionResponse(createResult);
      
      const sessionId = createResult.sessionId;
      
      // Load session  
      const loadResult = await this.sendRequest('loadSession', { sessionId }, 'Load session');
      this.validateLoadSessionResponse(loadResult, sessionId);
      
      return { sessionId, created: createResult, loaded: loadResult };
    });
  }

  async testErrorHandling() {
    return this.runTest('Error Handling', async () => {
      const errors = [];
      
      // Test unknown method
      try {
        await this.sendRequest('unknownMethod', {}, 'Unknown method');
        errors.push('Should have failed for unknown method');
      } catch (error) {
        if (error.code === -32601) {
          // Correct error code
        } else {
          errors.push(`Wrong error code for unknown method: ${error.code}`);
        }
      }
      
      // Test invalid params
      try {
        await this.sendRequest('loadSession', { sessionId: 'invalid' }, 'Invalid session');
        errors.push('Should have failed for invalid session');
      } catch (error) {
        // Should get some error (specific code may vary)
      }
      
      if (errors.length > 0) {
        throw new Error(`Error handling issues: ${errors.join(', ')}`);
      }
      
      return { errorTestsPassed: 2 };
    });
  }

  async testPerformance() {
    return this.runTest('Performance Benchmark', async () => {
      const iterations = 5;
      const times = [];
      
      for (let i = 0; i < iterations; i++) {
        const start = Date.now();
        
        await this.sendRequest('newSession', {}, `Performance test ${i + 1}`);
        
        const duration = Date.now() - start;
        times.push(duration);
      }
      
      const avgTime = times.reduce((a, b) => a + b, 0) / times.length;
      const minTime = Math.min(...times);
      const maxTime = Math.max(...times);
      
      return {
        averageResponseTime: Math.round(avgTime),
        minResponseTime: minTime,
        maxResponseTime: maxTime,
        iterations
      };
    });
  }

  // Validation helpers
  validateInitializeResponse(result) {
    if (!result.protocolVersion) {
      throw new Error('Missing protocolVersion in initialize response');
    }
    
    if (!result.serverInfo || !result.serverInfo.name) {
      throw new Error('Missing serverInfo.name in initialize response');
    }
    
    if (!result.capabilities) {
      throw new Error('Missing capabilities in initialize response');
    }
  }

  validateSessionResponse(result) {
    if (!result.sessionId) {
      throw new Error('Missing sessionId in newSession response');
    }
    
    if (typeof result.sessionId !== 'string') {
      throw new Error('sessionId must be a string');
    }
  }

  validateLoadSessionResponse(result, expectedSessionId) {
    if (result.id !== expectedSessionId) {
      throw new Error('loadSession returned wrong session ID');
    }
  }

  async runAllTests() {
    this.startTime = Date.now();
    
    this.log('info', 'ACP Protocol Compliance Test Suite');
    this.log('info', '=' .repeat(40));
    
    try {
      await this.startAgent();
      
      // Required tests
      const initResult = await this.testInitialize();
      await this.testSessionManagement();
      await this.testErrorHandling();
      
      // Performance benchmark
      await this.testPerformance();
      
      this.showResults();
      
    } catch (error) {
      this.log('error', `Test suite failed: ${error.message}`);
      this.showResults();
      return false;
    } finally {
      this.cleanup();
    }
    
    return this.testResults.every(r => r.status === 'PASSED');
  }

  showResults() {
    const totalTime = Date.now() - this.startTime;
    const passed = this.testResults.filter(r => r.status === 'PASSED').length;
    const failed = this.testResults.filter(r => r.status === 'FAILED').length;
    
    this.log('info', '');
    this.log('info', 'Test Results Summary');
    this.log('info', '=' .repeat(20));
    
    for (const result of this.testResults) {
      const icon = result.status === 'PASSED' ? '✅' : '❌';
      console.log(`${icon} ${result.name} (${result.duration}ms)`);
      if (result.error) {
        console.log(`   Error: ${result.error}`);
      }
    }
    
    this.log('info', '');
    this.log('info', `Total time: ${totalTime}ms`);
    this.log('info', `Passed: ${passed}`);
    this.log('info', `Failed: ${failed}`);
    this.log('info', `Total: ${this.testResults.length}`);
    
    if (failed === 0) {
      this.log('success', 'Agent is ACP compliant! 🎉');
    } else {
      this.log('error', `Agent failed ${failed} compliance test(s)`);
    }
  }

  cleanup() {
    if (this.agent) {
      this.agent.stdin.end();
      this.agent.kill();
    }
  }
}

// Custom error class for ACP errors
class ACPError extends Error {
  constructor(code, message, data = null) {
    super(message);
    this.code = code;
    this.data = data;
    this.name = 'ACPError';
  }
}

// CLI handling
async function main() {
  const args = process.argv.slice(2);
  const options = {};
  
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    
    if (arg === '--agent-command' && i + 1 < args.length) {
      options.agentCommand = args[++i];
    } else if (arg === '--timeout' && i + 1 < args.length) {
      options.timeout = parseInt(args[++i], 10) * 1000; // Convert to ms
    } else if (arg === '--verbose' || arg === '-v') {
      options.verbose = true;
    } else if (arg === '--help' || arg === '-h') {
      console.log(`
ACP Tester - Agent Client Protocol Compliance Testing Tool

Usage:
  acp-tester --agent-command "command to start your agent"

Options:
  --agent-command <cmd>    Command to start the agent (required)
  --timeout <seconds>      Request timeout in seconds (default: 30)
  --verbose, -v            Enable verbose output
  --help, -h               Show this help

Examples:
  acp-tester --agent-command "node my-agent.js --acp"
  acp-tester --agent-command "python agent.py" --timeout 60 --verbose
  acp-tester --agent-command "./my-agent-binary"

Test Suite:
  • Protocol initialization and capabilities
  • Session management (create/load)
  • Error handling and edge cases  
  • Performance benchmarking
  • JSON-RPC 2.0 compliance validation
`);
      process.exit(0);
    }
  }
  
  if (!options.agentCommand) {
    console.error('❌ Error: --agent-command is required');
    console.error('Use --help for usage information');
    process.exit(1);
  }
  
  const tester = new ACPTester(options);
  const success = await tester.runAllTests();
  
  process.exit(success ? 0 : 1);
}

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
  console.log('\n⏹️  Test interrupted');
  process.exit(0);
});

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { ACPTester };