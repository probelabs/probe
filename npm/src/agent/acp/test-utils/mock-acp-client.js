#!/usr/bin/env node

/**
 * Mock ACP Client - Simulates an editor/client communicating with ACP agents
 * 
 * This utility can be used to test ACP agents by simulating realistic 
 * editor interactions, including user prompts, tool usage, and session management.
 * 
 * Usage:
 *   mock-acp-client --agent "node agent.js --acp"
 *   mock-acp-client --agent "my-agent-binary" --interactive
 */

import { spawn } from 'child_process';
import { randomUUID } from 'crypto';
import { createInterface } from 'readline';

class MockACPClient {
  constructor(options = {}) {
    this.options = {
      agentCommand: options.agentCommand,
      interactive: options.interactive || false,
      timeout: options.timeout || 30000,
      debug: options.debug || false
    };
    
    this.agent = null;
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.buffer = '';
    this.sessionId = null;
    this.initialized = false;
    this.rl = null;
    
    if (this.options.interactive) {
      this.rl = createInterface({
        input: process.stdin,
        output: process.stdout
      });
    }
  }

  log(message, data = null) {
    const timestamp = new Date().toISOString().split('T')[1].split('.')[0];
    console.log(`[${timestamp}] ${message}`);
    
    if (data && this.options.debug) {
      console.log(JSON.stringify(data, null, 2));
    }
  }

  async startAgent() {
    if (!this.options.agentCommand) {
      throw new Error('Agent command required');
    }
    
    this.log(`🚀 Starting agent: ${this.options.agentCommand}`);
    
    const parts = this.options.agentCommand.split(' ');
    const command = parts[0];
    const args = parts.slice(1);
    
    this.agent = spawn(command, args, {
      stdio: ['pipe', 'pipe', 'pipe']
    });
    
    this.agent.stdout.on('data', (data) => {
      this.buffer += data.toString();
      this.processBuffer();
    });
    
    this.agent.stderr.on('data', (data) => {
      const output = data.toString().trim();
      if (output && this.options.debug) {
        this.log(`🔍 Agent debug: ${output}`);
      }
    });
    
    this.agent.on('close', (code) => {
      this.log(`📴 Agent closed with code ${code}`);
    });
    
    // Wait for agent to be ready
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
          if (this.options.debug) {
            this.log(`⚠️  Invalid JSON from agent: ${line.substring(0, 100)}...`);
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
        reject(new Error(`RPC Error ${message.error.code}: ${message.error.message}`));
      } else {
        resolve(message.result);
      }
    } else if (message.method) {
      // Notification from agent
      this.handleNotification(message);
    }
  }

  handleNotification(notification) {
    const { method, params } = notification;
    
    switch (method) {
      case 'sessionUpdated':
        this.log(`📝 Session updated: ${params.sessionId} → ${params.mode}`);
        break;
        
      case 'toolCallProgress':
        const { toolCallId, status, result, error } = params;
        const icon = {
          pending: '⏳',
          in_progress: '🔄',
          completed: '✅',
          failed: '❌'
        }[status] || '❓';
        
        this.log(`${icon} Tool ${toolCallId.substring(0, 8)}: ${status}`);
        
        if (status === 'completed' && result) {
          const preview = typeof result === 'string' 
            ? result.substring(0, 100) + (result.length > 100 ? '...' : '')
            : '[Object result]';
          this.log(`   📋 Result: ${preview}`);
        }
        
        if (status === 'failed' && error) {
          this.log(`   💥 Error: ${error}`);
        }
        break;
        
      case 'messageChunk':
        // Streaming response chunk
        process.stdout.write(params.chunk);
        break;
        
      default:
        this.log(`📨 Notification: ${method}`, params);
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
      
      this.agent.stdin.write(JSON.stringify(message) + '\n');
      
      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error('Request timeout'));
        }
      }, this.options.timeout);
    });
  }

  async initialize() {
    this.log('🔧 Initializing ACP connection...');
    
    const result = await this.sendRequest('initialize', {
      protocolVersion: '1'
    });
    
    this.initialized = true;
    
    this.log(`✅ Connected to ${result.serverInfo.name} v${result.serverInfo.version}`);
    this.log(`🛠️  Available tools: ${result.capabilities.tools.map(t => t.name).join(', ')}`);
    
    return result;
  }

  async createSession() {
    this.log('📝 Creating new session...');
    
    const result = await this.sendRequest('newSession', {});
    this.sessionId = result.sessionId;
    
    this.log(`✅ Session created: ${this.sessionId}`);
    
    return result;
  }

  async sendPrompt(message) {
    if (!this.sessionId) {
      throw new Error('No active session');
    }
    
    this.log(`💬 Sending: "${message}"`);
    
    const result = await this.sendRequest('prompt', {
      sessionId: this.sessionId,
      message
    });
    
    // Display response
    this.log('🤖 Agent response:');
    for (const block of result.content) {
      if (block.type === 'text') {
        console.log('━'.repeat(60));
        console.log(block.text);
        console.log('━'.repeat(60));
      }
    }
    console.log();
    
    return result;
  }

  async setSessionMode(mode) {
    if (!this.sessionId) {
      throw new Error('No active session');
    }
    
    this.log(`⚙️  Setting session mode to: ${mode}`);
    
    const result = await this.sendRequest('setSessionMode', {
      sessionId: this.sessionId,
      mode
    });
    
    return result;
  }

  async runInteractiveSession() {
    this.log('🎮 Starting interactive session...');
    this.log('Type "help" for commands, "quit" to exit');
    console.log();
    
    while (true) {
      const input = await this.prompt('> ');
      const trimmed = input.trim();
      
      if (!trimmed) continue;
      
      if (trimmed === 'quit' || trimmed === 'exit') {
        break;
      }
      
      if (trimmed === 'help') {
        this.showHelp();
        continue;
      }
      
      if (trimmed.startsWith('mode ')) {
        const mode = trimmed.substring(5);
        try {
          await this.setSessionMode(mode);
        } catch (error) {
          this.log(`❌ Mode change failed: ${error.message}`);
        }
        continue;
      }
      
      if (trimmed === 'status') {
        this.log(`Session: ${this.sessionId}`);
        this.log(`Initialized: ${this.initialized}`);
        continue;
      }
      
      // Default: send as prompt
      try {
        await this.sendPrompt(trimmed);
      } catch (error) {
        this.log(`❌ Prompt failed: ${error.message}`);
      }
    }
  }

  showHelp() {
    console.log(`
Available commands:
  help          Show this help
  quit, exit    Exit the client
  mode <mode>   Set session mode (normal, planning)
  status        Show connection status
  
  Any other text will be sent as a prompt to the agent.
`);
  }

  async prompt(question) {
    return new Promise(resolve => {
      this.rl.question(question, resolve);
    });
  }

  async runDemoSession() {
    this.log('🎬 Running demo session...');
    
    // Demo conversation
    const prompts = [
      'Hello! Can you tell me about yourself?',
      'What files are in this project?',
      'Find all TypeScript files',
      'Show me the main entry point'
    ];
    
    for (const prompt of prompts) {
      try {
        await this.sendPrompt(prompt);
        await new Promise(resolve => setTimeout(resolve, 1000));
      } catch (error) {
        this.log(`❌ Demo prompt failed: ${error.message}`);
        if (error.message.includes('API key')) {
          this.log('ℹ️  Demo requires AI API keys to be set');
          break;
        }
      }
    }
  }

  async run() {
    try {
      await this.startAgent();
      await this.initialize();
      await this.createSession();
      
      if (this.options.interactive) {
        await this.runInteractiveSession();
      } else {
        await this.runDemoSession();
      }
      
    } catch (error) {
      this.log(`❌ Client error: ${error.message}`);
    } finally {
      this.cleanup();
    }
  }

  cleanup() {
    if (this.rl) {
      this.rl.close();
    }
    
    if (this.agent) {
      this.agent.stdin.end();
      this.agent.kill();
    }
  }
}

// CLI handling
async function main() {
  const args = process.argv.slice(2);
  const options = {};
  
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    
    if (arg === '--agent' && i + 1 < args.length) {
      options.agentCommand = args[++i];
    } else if (arg === '--interactive' || arg === '-i') {
      options.interactive = true;
    } else if (arg === '--debug' || arg === '-d') {
      options.debug = true;
    } else if (arg === '--timeout' && i + 1 < args.length) {
      options.timeout = parseInt(args[++i], 10) * 1000;
    } else if (arg === '--help' || arg === '-h') {
      console.log(`
Mock ACP Client - Test tool for ACP agents

Usage:
  mock-acp-client --agent "command to start agent"

Options:
  --agent <command>        Command to start the ACP agent (required)
  --interactive, -i        Start interactive session
  --debug, -d              Enable debug output  
  --timeout <seconds>      Request timeout (default: 30)
  --help, -h               Show this help

Examples:
  mock-acp-client --agent "node agent.js --acp"
  mock-acp-client --agent "python my_agent.py" --interactive
  mock-acp-client --agent "./agent-binary" --debug
`);
      process.exit(0);
    }
  }
  
  if (!options.agentCommand) {
    console.error('❌ Error: --agent is required');
    console.error('Use --help for usage information');
    process.exit(1);
  }
  
  const client = new MockACPClient(options);
  await client.run();
}

// Handle Ctrl+C gracefully  
process.on('SIGINT', () => {
  console.log('\n👋 Goodbye!');
  process.exit(0);
});

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { MockACPClient };