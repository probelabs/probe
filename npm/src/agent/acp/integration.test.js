// Integration tests for ACP implementation
import { jest } from '@jest/globals';
import { EventEmitter } from 'events';
import { ACPServer } from './server.js';
import { ACPConnection } from './connection.js';
import { ACP_PROTOCOL_VERSION, RequestMethod } from './types.js';

// Mock manually handled below

// Mock streams for testing
class MockStream extends EventEmitter {
  constructor() {
    super();
    this.encoding = null;
    this.writtenData = [];
  }
  
  setEncoding(encoding) {
    this.encoding = encoding;
  }
  
  write(data) {
    this.writtenData.push(data);
    return true;
  }
}

describe('ACP Integration', () => {
  let server, clientInput, clientOutput, serverInput, serverOutput;
  let mockProbeAgent;
  
  beforeEach(async () => {
    // Setup mock streams - simulate client/server communication
    clientInput = new MockStream();   // Client receives from server
    clientOutput = new MockStream();  // Client sends to server
    serverInput = new MockStream();   // Server receives from client
    serverOutput = new MockStream();  // Server sends to client
    
    // Connect the streams
    clientOutput.pipe = jest.fn();
    serverOutput.pipe = jest.fn();
    
    // Mock ProbeAgent
    const { ProbeAgent } = await import('../ProbeAgent.js');
    mockProbeAgent = {
      answer: jest.fn().mockResolvedValue('Test AI response'),
      cancel: jest.fn(),
      sessionId: 'test-session'
    };
    ProbeAgent.mockImplementation(() => mockProbeAgent);
    
    // Create server with mock streams
    server = new ACPServer({ debug: false });
    server.connection = new ACPConnection(serverInput, serverOutput);
  });
  
  afterEach(() => {
    if (server.connection) {
      server.connection.close();
    }
  });
  
  describe('full protocol flow', () => {
    test('should handle complete initialization flow', async () => {
      // Start server
      server.connection.start();
      
      // Simulate client initialization request
      const initRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.INITIALIZE,
        params: { protocolVersion: ACP_PROTOCOL_VERSION },
        id: 1
      };
      
      // Simulate receiving the request
      serverInput.emit('data', JSON.stringify(initRequest) + '\n');
      
      // Wait for async processing
      await new Promise(resolve => setTimeout(resolve, 10));
      
      // Check that server sent response
      expect(serverOutput.writtenData).toHaveLength(1);
      const response = JSON.parse(serverOutput.writtenData[0]);
      
      expect(response).toEqual({
        jsonrpc: '2.0',
        id: 1,
        result: {
          protocolVersion: ACP_PROTOCOL_VERSION,
          serverInfo: {
            name: 'probe-agent-acp',
            version: '1.0.0',
            description: 'Probe AI agent with code search capabilities'
          },
          capabilities: server.capabilities
        }
      });
      
      expect(server.initialized).toBe(true);
    });
    
    test('should handle session creation and prompt flow', async () => {
      server.connection.start();
      server.initialized = true; // Skip initialization for this test
      
      // 1. Create new session
      const newSessionRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.NEW_SESSION,
        params: {},
        id: 2
      };
      
      serverInput.emit('data', JSON.stringify(newSessionRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      expect(serverOutput.writtenData).toHaveLength(1);
      const sessionResponse = JSON.parse(serverOutput.writtenData[0]);
      expect(sessionResponse.result.sessionId).toBeDefined();
      const sessionId = sessionResponse.result.sessionId;
      
      // Clear previous data
      serverOutput.writtenData = [];
      
      // 2. Send prompt request
      const promptRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.PROMPT,
        params: {
          sessionId,
          message: 'How does authentication work in this codebase?'
        },
        id: 3
      };
      
      serverInput.emit('data', JSON.stringify(promptRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 50)); // Give more time for AI processing
      
      // Check that AI was called
      expect(mockProbeAgent.answer).toHaveBeenCalledWith('How does authentication work in this codebase?');
      
      // Check response
      expect(serverOutput.writtenData).toHaveLength(1);
      const promptResponse = JSON.parse(serverOutput.writtenData[0]);
      
      expect(promptResponse).toEqual({
        jsonrpc: '2.0',
        id: 3,
        result: {
          content: [{ type: 'text', text: 'Test AI response' }],
          sessionId,
          timestamp: expect.any(String)
        }
      });
      
      // Check that session history was updated
      const session = server.sessions.get(sessionId);
      expect(session.history).toHaveLength(2);
      expect(session.history[0].role).toBe('user');
      expect(session.history[1].role).toBe('assistant');
    });
    
    test('should handle errors gracefully', async () => {
      server.connection.start();
      
      // Send invalid method
      const invalidRequest = {
        jsonrpc: '2.0',
        method: 'invalidMethod',
        params: {},
        id: 4
      };
      
      serverInput.emit('data', JSON.stringify(invalidRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      expect(serverOutput.writtenData).toHaveLength(1);
      const errorResponse = JSON.parse(serverOutput.writtenData[0]);
      
      expect(errorResponse).toEqual({
        jsonrpc: '2.0',
        id: 4,
        error: {
          code: -32601,
          message: 'Unknown method: invalidMethod'
        }
      });
    });
    
    test('should handle session mode changes with notifications', async () => {
      server.connection.start();
      server.initialized = true;
      
      // Create session
      const newSessionRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.NEW_SESSION,
        params: {},
        id: 5
      };
      
      serverInput.emit('data', JSON.stringify(newSessionRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      const sessionResponse = JSON.parse(serverOutput.writtenData[0]);
      const sessionId = sessionResponse.result.sessionId;
      
      // Clear previous data
      serverOutput.writtenData = [];
      
      // Set session mode
      const setModeRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.SET_SESSION_MODE,
        params: {
          sessionId,
          mode: 'planning'
        },
        id: 6
      };
      
      serverInput.emit('data', JSON.stringify(setModeRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      // Should have both response and notification
      expect(serverOutput.writtenData).toHaveLength(2);
      
      // Check response
      const modeResponse = JSON.parse(serverOutput.writtenData[0]);
      expect(modeResponse).toEqual({
        jsonrpc: '2.0',
        id: 6,
        result: { success: true }
      });
      
      // Check notification
      const notification = JSON.parse(serverOutput.writtenData[1]);
      expect(notification).toEqual({
        jsonrpc: '2.0',
        method: 'sessionUpdated',
        params: {
          sessionId,
          mode: 'planning'
        }
      });
    });
    
    test('should handle cancellation', async () => {
      server.connection.start();
      server.initialized = true;
      
      // Create session and trigger agent creation
      const newSessionRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.NEW_SESSION,
        params: {},
        id: 7
      };
      
      serverInput.emit('data', JSON.stringify(newSessionRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      const sessionResponse = JSON.parse(serverOutput.writtenData[0]);
      const sessionId = sessionResponse.result.sessionId;
      
      // Send a prompt to create the agent
      const promptRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.PROMPT,
        params: {
          sessionId,
          message: 'Test question'
        },
        id: 8
      };
      
      serverInput.emit('data', JSON.stringify(promptRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      // Clear previous data
      serverOutput.writtenData = [];
      
      // Send cancel request
      const cancelRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.CANCEL,
        params: { sessionId },
        id: 9
      };
      
      serverInput.emit('data', JSON.stringify(cancelRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      // Check that cancel was called on agent
      expect(mockProbeAgent.cancel).toHaveBeenCalled();
      
      // Check response
      expect(serverOutput.writtenData).toHaveLength(1);
      const cancelResponse = JSON.parse(serverOutput.writtenData[0]);
      expect(cancelResponse).toEqual({
        jsonrpc: '2.0',
        id: 9,
        result: { success: true }
      });
    });
  });
  
  describe('error handling', () => {
    beforeEach(() => {
      server.connection.start();
    });
    
    test('should handle malformed JSON', async () => {
      serverInput.emit('data', 'malformed json\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      expect(serverOutput.writtenData).toHaveLength(1);
      const errorResponse = JSON.parse(serverOutput.writtenData[0]);
      expect(errorResponse.error.code).toBe(-32700); // Parse error
    });
    
    test('should handle invalid JSON-RPC format', async () => {
      const invalidMessage = { jsonrpc: '1.0', method: 'test' };
      serverInput.emit('data', JSON.stringify(invalidMessage) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      expect(serverOutput.writtenData).toHaveLength(1);
      const errorResponse = JSON.parse(serverOutput.writtenData[0]);
      expect(errorResponse.error.code).toBe(-32600); // Invalid request
    });
    
    test('should handle AI errors in prompt processing', async () => {
      server.initialized = true;
      
      // Mock AI to throw error
      mockProbeAgent.answer.mockRejectedValue(new Error('AI service unavailable'));
      
      // Create session
      await server.handleNewSession({ sessionId: 'test-session' });
      
      const promptRequest = {
        jsonrpc: '2.0',
        method: RequestMethod.PROMPT,
        params: {
          sessionId: 'test-session',
          message: 'Test question'
        },
        id: 10
      };
      
      serverInput.emit('data', JSON.stringify(promptRequest) + '\n');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      expect(serverOutput.writtenData).toHaveLength(1);
      const response = JSON.parse(serverOutput.writtenData[0]);
      
      expect(response.result.content[0].text).toBe('Error: AI service unavailable');
      expect(response.result.error).toBe(true);
    });
  });
  
  describe('connection management', () => {
    test('should clean up on disconnect', async () => {
      server.connection.start();
      server.initialized = true;
      
      // Create some sessions
      await server.handleNewSession({ sessionId: 'session1' });
      await server.handleNewSession({ sessionId: 'session2' });
      await server.handlePrompt({ sessionId: 'session1', message: 'test' });
      await server.handlePrompt({ sessionId: 'session2', message: 'test' });
      
      expect(server.sessions.size).toBe(2);
      
      // Simulate disconnect
      serverInput.emit('end');
      await new Promise(resolve => setTimeout(resolve, 10));
      
      // All sessions should be cleaned up
      expect(server.sessions.size).toBe(0);
      expect(mockProbeAgent.cancel).toHaveBeenCalledTimes(2);
    });
  });
});