#!/usr/bin/env node

/**
 * Simple ACP Client Example
 * 
 * This example demonstrates how to communicate with the Probe ACP server
 * using basic JSON-RPC 2.0 messages over stdio.
 * 
 * Usage:
 *   node simple-client.js
 * 
 * Make sure to start the ACP server first:
 *   probe agent --acp
 */

import { spawn } from 'child_process';
import { EventEmitter } from 'events';

class SimpleACPClient extends EventEmitter {
  constructor() {
    super();
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.sessionId = null;
    this.server = null;
  }

  async start() {
    console.log('🚀 Starting Probe ACP server...');
    
    // Spawn the ACP server
    this.server = spawn('node', ['../../../index.js', '--acp'], {
      stdio: ['pipe', 'pipe', 'pipe']
    });
    
    // Handle server output (our responses)
    this.server.stdout.on('data', (data) => {
      const lines = data.toString().split('\n').filter(line => line.trim());
      for (const line of lines) {
        try {
          const message = JSON.parse(line);
          this.handleMessage(message);
        } catch (error) {
          console.error('❌ Failed to parse server message:', line);
        }
      }
    });
    
    // Handle server errors
    this.server.stderr.on('data', (data) => {
      const output = data.toString();
      if (output.includes('[ACP]') || output.includes('DEBUG')) {
        console.log('🔍 Server:', output.trim());
      } else {
        console.error('⚠️  Server error:', output.trim());
      }
    });
    
    this.server.on('close', (code) => {
      console.log(`📴 Server closed with code ${code}`);
    });
    
    // Wait a bit for server to start
    await new Promise(resolve => setTimeout(resolve, 1000));
    console.log('✅ Server started\n');
  }

  handleMessage(message) {
    if (message.id && this.pendingRequests.has(message.id)) {
      // This is a response to our request
      const { resolve, reject } = this.pendingRequests.get(message.id);
      this.pendingRequests.delete(message.id);
      
      if (message.error) {
        reject(new Error(`${message.error.code}: ${message.error.message}`));
      } else {
        resolve(message.result);
      }
    } else if (message.method) {
      // This is a notification from the server
      console.log(`📨 Notification: ${message.method}`, message.params);
    }
  }

  async sendRequest(method, params = null) {
    const id = this.messageId++;
    const message = {
      jsonrpc: '2.0',
      method,
      id
    };
    
    if (params !== null) {
      message.params = params;
    }
    
    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      
      // Send message to server
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

  async initialize() {
    console.log('🔧 Initializing ACP protocol...');
    const result = await this.sendRequest('initialize', {
      protocolVersion: '1'
    });
    
    console.log('✅ Protocol initialized');
    console.log(`   Server: ${result.serverInfo.name} v${result.serverInfo.version}`);
    console.log(`   Capabilities: ${result.capabilities.tools.length} tools, sessions: ${result.capabilities.sessionManagement}`);
    console.log();
    
    return result;
  }

  async createSession() {
    console.log('📝 Creating new session...');
    const result = await this.sendRequest('newSession', {});
    
    this.sessionId = result.sessionId;
    console.log(`✅ Session created: ${this.sessionId}`);
    console.log();
    
    return result;
  }

  async sendPrompt(message) {
    if (!this.sessionId) {
      throw new Error('No active session. Create a session first.');
    }
    
    console.log(`💬 Sending prompt: "${message}"`);
    console.log('🤖 AI is thinking...\n');
    
    const result = await this.sendRequest('prompt', {
      sessionId: this.sessionId,
      message
    });
    
    // Extract text from content blocks
    const text = result.content
      .filter(block => block.type === 'text')
      .map(block => block.text)
      .join('\n');
    
    console.log('🤖 AI Response:');
    console.log('━'.repeat(50));
    console.log(text);
    console.log('━'.repeat(50));
    console.log();
    
    return result;
  }

  async close() {
    console.log('🔴 Closing connection...');
    if (this.server) {
      this.server.stdin.end();
      this.server.kill();
    }
  }
}

// Example usage
async function main() {
  const client = new SimpleACPClient();
  
  try {
    await client.start();
    await client.initialize();
    await client.createSession();
    
    // Example conversations
    await client.sendPrompt("What files are in this project?");
    
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await client.sendPrompt("Find all functions that handle HTTP requests");
    
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await client.sendPrompt("Show me the main entry point of this application");
    
  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    await client.close();
    process.exit(0);
  }
}

// Handle Ctrl+C gracefully
process.on('SIGINT', async () => {
  console.log('\n👋 Goodbye!');
  process.exit(0);
});

// Run the example
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}