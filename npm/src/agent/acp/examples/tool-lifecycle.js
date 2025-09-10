#!/usr/bin/env node

/**
 * ACP Tool Lifecycle Example
 * 
 * This example demonstrates how ACP tracks tool execution lifecycle
 * with status notifications for search, query, and extract operations.
 * 
 * Usage:
 *   node tool-lifecycle.js
 */

import { spawn } from 'child_process';

class ACPToolLifecycleDemo {
  constructor() {
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.sessionId = null;
    this.server = null;
    this.toolCalls = new Map();
  }

  async start() {
    console.log('🛠️  ACP Tool Lifecycle Demo');
    console.log('=' .repeat(40));
    console.log('This demo shows how ACP tracks tool execution with status updates\n');
    
    // Start server
    this.server = spawn('node', ['../../../index.js', '--acp', '--verbose'], {
      stdio: ['pipe', 'pipe', 'pipe']
    });
    
    this.server.stdout.on('data', (data) => {
      const lines = data.toString().split('\n').filter(line => line.trim());
      for (const line of lines) {
        try {
          const message = JSON.parse(line);
          this.handleMessage(message);
        } catch (error) {
          // Ignore parse errors for non-JSON output
        }
      }
    });
    
    this.server.stderr.on('data', (data) => {
      // Log server debug output
      const output = data.toString().trim();
      if (output && !output.includes('Token usage')) {
        console.log(`🔍 ${output}`);
      }
    });
    
    await new Promise(resolve => setTimeout(resolve, 1000));
  }

  handleMessage(message) {
    if (message.id && this.pendingRequests.has(message.id)) {
      // Response to our request
      const { resolve, reject } = this.pendingRequests.get(message.id);
      this.pendingRequests.delete(message.id);
      
      if (message.error) {
        reject(new Error(`${message.error.code}: ${message.error.message}`));
      } else {
        resolve(message.result);
      }
    } else if (message.method === 'toolCallProgress') {
      // Tool lifecycle notification
      this.handleToolProgress(message.params);
    }
  }

  handleToolProgress(params) {
    const { toolCallId, status, sessionId, result, error } = params;
    
    // Track tool call
    if (!this.toolCalls.has(toolCallId)) {
      this.toolCalls.set(toolCallId, {
        id: toolCallId,
        sessionId,
        startTime: Date.now(),
        status: 'pending'
      });
    }
    
    const toolCall = this.toolCalls.get(toolCallId);
    toolCall.status = status;
    toolCall.endTime = Date.now();
    toolCall.duration = toolCall.endTime - toolCall.startTime;
    
    // Display status update
    const statusIcon = {
      pending: '⏳',
      in_progress: '🔄', 
      completed: '✅',
      failed: '❌'
    }[status] || '❓';
    
    console.log(`${statusIcon} Tool ${toolCallId.slice(0, 8)}: ${status.toUpperCase()}`);
    
    if (status === 'completed' && result) {
      const preview = typeof result === 'string' 
        ? result.slice(0, 100) + (result.length > 100 ? '...' : '')
        : 'Object result';
      console.log(`   📋 Result: ${preview}`);
    }
    
    if (status === 'failed' && error) {
      console.log(`   💥 Error: ${error}`);
    }
    
    if (status === 'completed' || status === 'failed') {
      console.log(`   ⏱️  Duration: ${toolCall.duration}ms`);
    }
    
    console.log();
  }

  async sendRequest(method, params = null) {
    const id = this.messageId++;
    const message = { jsonrpc: '2.0', method, id };
    if (params) message.params = params;
    
    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      this.server.stdin.write(JSON.stringify(message) + '\n');
      
      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error('Timeout'));
        }
      }, 30000);
    });
  }

  async setup() {
    await this.sendRequest('initialize', { protocolVersion: '1' });
    const session = await this.sendRequest('newSession', {});
    this.sessionId = session.sessionId;
    console.log(`📝 Session created: ${this.sessionId}\n`);
  }

  async demonstrateToolLifecycle() {
    console.log('🔍 Demonstrating Search Tool Lifecycle');
    console.log('-'.repeat(40));
    
    await this.sendRequest('prompt', {
      sessionId: this.sessionId,
      message: 'Search for functions named "search" in the codebase'
    });
    
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    console.log('🎯 Demonstrating Query Tool Lifecycle');
    console.log('-'.repeat(40));
    
    await this.sendRequest('prompt', {
      sessionId: this.sessionId, 
      message: 'Find all function definitions using structural patterns'
    });
    
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    console.log('📤 Demonstrating Extract Tool Lifecycle');
    console.log('-'.repeat(40));
    
    await this.sendRequest('prompt', {
      sessionId: this.sessionId,
      message: 'Extract the main function from the entry point file'
    });
    
    await new Promise(resolve => setTimeout(resolve, 3000));
  }

  async showStatistics() {
    console.log('\n📊 Tool Execution Statistics');
    console.log('='.repeat(40));
    
    const completedCalls = Array.from(this.toolCalls.values())
      .filter(call => call.status === 'completed');
    
    const failedCalls = Array.from(this.toolCalls.values())
      .filter(call => call.status === 'failed');
    
    console.log(`Total tool calls: ${this.toolCalls.size}`);
    console.log(`Completed: ${completedCalls.length}`);
    console.log(`Failed: ${failedCalls.length}`);
    
    if (completedCalls.length > 0) {
      const avgDuration = completedCalls
        .reduce((sum, call) => sum + call.duration, 0) / completedCalls.length;
      console.log(`Average duration: ${Math.round(avgDuration)}ms`);
    }
    
    console.log();
  }

  async close() {
    this.server?.kill();
  }
}

async function main() {
  const demo = new ACPToolLifecycleDemo();
  
  try {
    await demo.start();
    await demo.setup();
    await demo.demonstrateToolLifecycle();
    await demo.showStatistics();
    
  } catch (error) {
    console.error('❌ Demo failed:', error.message);
  } finally {
    await demo.close();
    console.log('👋 Demo completed!');
    process.exit(0);
  }
}

process.on('SIGINT', () => {
  console.log('\n⏹️  Demo stopped');
  process.exit(0);
});

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}