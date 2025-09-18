// Tests for ACP Server
import { jest } from '@jest/globals';
import { ACPServer } from './server.js';
import { ACPConnection } from './connection.js';
import { ACP_PROTOCOL_VERSION, RequestMethod, SessionMode, ErrorCode } from './types.js';

// Mock manually handled below

describe('ACPServer', () => {
  let server;
  let mockConnection;
  
  beforeEach(() => {
    // Reset mocks
    jest.clearAllMocks();
    
    // Create mock connection
    mockConnection = {
      start: jest.fn(),
      sendResponse: jest.fn(),
      sendError: jest.fn(),
      sendNotification: jest.fn(),
      on: jest.fn()
    };
    
    // Mock the ACPConnection constructor
    ACPConnection.mockImplementation(() => mockConnection);
    
    server = new ACPServer({
      debug: true,
      provider: 'anthropic',
      path: '/test/path'
    });
  });
  
  describe('initialization', () => {
    test('should create server with default options', () => {
      const defaultServer = new ACPServer();
      expect(defaultServer.options.debug).toBe(false);
      expect(defaultServer.sessions.size).toBe(0);
      expect(defaultServer.initialized).toBe(false);
    });
    
    test('should create server with custom options', () => {
      expect(server.options.debug).toBe(true);
      expect(server.options.provider).toBe('anthropic');
      expect(server.options.path).toBe('/test/path');
    });
    
    test('should have correct capabilities', () => {
      const capabilities = server.getCapabilities();
      expect(capabilities.tools).toHaveLength(3);
      expect(capabilities.tools[0].name).toBe('search');
      expect(capabilities.tools[1].name).toBe('query');
      expect(capabilities.tools[2].name).toBe('extract');
      expect(capabilities.sessionManagement).toBe(true);
      expect(capabilities.streaming).toBe(true);
    });
  });
  
  describe('server startup', () => {
    test('should start server and setup connection', async () => {
      await server.start();
      
      expect(ACPConnection).toHaveBeenCalledWith(process.stdin, process.stderr);
      expect(mockConnection.on).toHaveBeenCalledWith('request', expect.any(Function));
      expect(mockConnection.on).toHaveBeenCalledWith('notification', expect.any(Function));
      expect(mockConnection.on).toHaveBeenCalledWith('error', expect.any(Function));
      expect(mockConnection.on).toHaveBeenCalledWith('disconnect', expect.any(Function));
      expect(mockConnection.start).toHaveBeenCalled();
    });
  });
  
  describe('initialize request', () => {
    test('should handle valid initialize request', async () => {
      const params = { protocolVersion: ACP_PROTOCOL_VERSION };
      
      const result = await server.handleInitialize(params);
      
      expect(result).toEqual({
        protocolVersion: ACP_PROTOCOL_VERSION,
        serverInfo: {
          name: 'probe-agent-acp',
          version: '1.0.0',
          description: 'Probe AI agent with code search capabilities'
        },
        capabilities: server.capabilities
      });
      expect(server.initialized).toBe(true);
    });
    
    test('should reject invalid protocol version', async () => {
      const params = { protocolVersion: '2.0' };
      
      await expect(server.handleInitialize(params)).rejects.toThrow('Unsupported protocol version: 2.0');
    });
    
    test('should require protocolVersion parameter', async () => {
      await expect(server.handleInitialize({})).rejects.toThrow('Invalid params: protocolVersion required');
      await expect(server.handleInitialize(null)).rejects.toThrow('Invalid params: protocolVersion required');
    });
  });
  
  describe('session management', () => {
    test('should create new session', async () => {
      const result = await server.handleNewSession({});
      
      expect(result.sessionId).toBeDefined();
      expect(result.mode).toBe(SessionMode.NORMAL);
      expect(result.createdAt).toBeDefined();
      expect(server.sessions.has(result.sessionId)).toBe(true);
    });
    
    test('should create session with custom ID and mode', async () => {
      const customId = 'test-session-123';
      const params = {
        sessionId: customId,
        mode: SessionMode.PLANNING
      };
      
      const result = await server.handleNewSession(params);
      
      expect(result.sessionId).toBe(customId);
      expect(result.mode).toBe(SessionMode.PLANNING);
      expect(server.sessions.has(customId)).toBe(true);
    });
    
    test('should load existing session', async () => {
      // Create a session first
      const createResult = await server.handleNewSession({});
      const sessionId = createResult.sessionId;
      
      // Load the session
      const loadResult = await server.handleLoadSession({ sessionId });
      
      expect(loadResult.id).toBe(sessionId);
      expect(loadResult.mode).toBe(SessionMode.NORMAL);
      expect(loadResult.historyLength).toBe(0);
      expect(loadResult.toolCallsCount).toBe(0);
    });
    
    test('should fail to load non-existent session', async () => {
      const params = { sessionId: 'non-existent' };
      
      await expect(server.handleLoadSession(params)).rejects.toThrow('Session not found: non-existent');
    });
    
    test('should set session mode', async () => {
      // Create a session first
      const createResult = await server.handleNewSession({});
      const sessionId = createResult.sessionId;
      
      // Set mode
      const result = await server.handleSetSessionMode({
        sessionId,
        mode: SessionMode.PLANNING
      });
      
      expect(result.success).toBe(true);
      expect(mockConnection.sendNotification).toHaveBeenCalledWith(
        'sessionUpdated',
        { sessionId, mode: SessionMode.PLANNING }
      );
      
      const session = server.sessions.get(sessionId);
      expect(session.mode).toBe(SessionMode.PLANNING);
    });
  });
  
  describe('request handling', () => {
    test('should handle requests and send responses', async () => {
      const message = {
        method: RequestMethod.INITIALIZE,
        params: { protocolVersion: ACP_PROTOCOL_VERSION },
        id: 1
      };
      
      await server.handleRequest(message);
      
      expect(mockConnection.sendResponse).toHaveBeenCalledWith(1, expect.objectContaining({
        protocolVersion: ACP_PROTOCOL_VERSION
      }));
    });
    
    test('should handle unknown methods', async () => {
      const message = {
        method: 'unknownMethod',
        params: {},
        id: 2
      };
      
      await server.handleRequest(message);
      
      expect(mockConnection.sendError).toHaveBeenCalledWith(
        2,
        ErrorCode.METHOD_NOT_FOUND,
        'Unknown method: unknownMethod'
      );
    });
    
    test('should handle request errors', async () => {
      const message = {
        method: RequestMethod.LOAD_SESSION,
        params: { sessionId: 'invalid' },
        id: 3
      };
      
      await server.handleRequest(message);
      
      expect(mockConnection.sendError).toHaveBeenCalledWith(
        3,
        ErrorCode.INTERNAL_ERROR,
        'Session not found: invalid'
      );
    });
  });
  
  describe('prompt handling', () => {
    let mockAgent;
    
    beforeEach(async () => {
      // Import and mock ProbeAgent
      const { ProbeAgent } = await import('../ProbeAgent.js');
      mockAgent = {
        answer: jest.fn().mockResolvedValue('Test response'),
        cancel: jest.fn()
      };
      ProbeAgent.mockImplementation(() => mockAgent);
      
      // Create a session
      await server.handleNewSession({ sessionId: 'test-session' });
    });
    
    test('should handle prompt request successfully', async () => {
      const params = {
        sessionId: 'test-session',
        message: 'Test question'
      };
      
      const result = await server.handlePrompt(params);
      
      expect(mockAgent.answer).toHaveBeenCalledWith('Test question');
      expect(result).toEqual({
        content: [{ type: 'text', text: 'Test response' }],
        sessionId: 'test-session',
        timestamp: expect.any(String)
      });
      
      // Check that history was updated
      const session = server.sessions.get('test-session');
      expect(session.history).toHaveLength(2);
      expect(session.history[0].role).toBe('user');
      expect(session.history[1].role).toBe('assistant');
    });
    
    test('should handle prompt errors gracefully', async () => {
      mockAgent.answer.mockRejectedValue(new Error('AI Error'));
      
      const params = {
        sessionId: 'test-session',
        message: 'Test question'
      };
      
      const result = await server.handlePrompt(params);
      
      expect(result).toEqual({
        content: [{ type: 'text', text: 'Error: AI Error' }],
        sessionId: 'test-session',
        timestamp: expect.any(String),
        error: true
      });
    });
    
    test('should require sessionId and message', async () => {
      await expect(server.handlePrompt({})).rejects.toThrow('Invalid params: sessionId and message required');
      await expect(server.handlePrompt({ sessionId: 'test' })).rejects.toThrow('Invalid params: sessionId and message required');
      await expect(server.handlePrompt({ message: 'test' })).rejects.toThrow('Invalid params: sessionId and message required');
    });
    
    test('should require valid session', async () => {
      const params = {
        sessionId: 'invalid-session',
        message: 'Test question'
      };
      
      await expect(server.handlePrompt(params)).rejects.toThrow('Session not found: invalid-session');
    });
  });
  
  describe('cancel handling', () => {
    let mockAgent;
    
    beforeEach(async () => {
      const { ProbeAgent } = await import('../ProbeAgent.js');
      mockAgent = {
        answer: jest.fn().mockResolvedValue('Test response'),
        cancel: jest.fn()
      };
      ProbeAgent.mockImplementation(() => mockAgent);
      
      // Create a session and trigger agent creation
      await server.handleNewSession({ sessionId: 'test-session' });
      await server.handlePrompt({
        sessionId: 'test-session',
        message: 'Test question'
      });
    });
    
    test('should cancel session operations', async () => {
      const result = await server.handleCancel({ sessionId: 'test-session' });
      
      expect(mockAgent.cancel).toHaveBeenCalled();
      expect(result.success).toBe(true);
    });
    
    test('should handle cancel for session without agent', async () => {
      await server.handleNewSession({ sessionId: 'new-session' });
      
      const result = await server.handleCancel({ sessionId: 'new-session' });
      expect(result.success).toBe(true);
    });
    
    test('should require sessionId for cancel', async () => {
      await expect(server.handleCancel({})).rejects.toThrow('Invalid params: sessionId required');
    });
  });
  
  describe('disconnect handling', () => {
    let mockAgent;
    
    beforeEach(async () => {
      const { ProbeAgent } = await import('../ProbeAgent.js');
      mockAgent = {
        answer: jest.fn().mockResolvedValue('Test response'),
        cancel: jest.fn()
      };
      ProbeAgent.mockImplementation(() => mockAgent);
      
      // Create sessions with agents
      await server.handleNewSession({ sessionId: 'session1' });
      await server.handleNewSession({ sessionId: 'session2' });
      await server.handlePrompt({ sessionId: 'session1', message: 'test' });
      await server.handlePrompt({ sessionId: 'session2', message: 'test' });
    });
    
    test('should clean up sessions and cancel agents on disconnect', () => {
      expect(server.sessions.size).toBe(2);
      
      server.handleDisconnect();
      
      expect(mockAgent.cancel).toHaveBeenCalledTimes(2);
      expect(server.sessions.size).toBe(0);
    });
  });
  
  describe('stats', () => {
    test('should return server statistics', async () => {
      await server.handleNewSession({});
      await server.handleNewSession({});
      server.initialized = true;
      
      const stats = server.getStats();
      
      expect(stats).toEqual({
        sessions: 2,
        initialized: true,
        capabilities: server.capabilities
      });
    });
  });
});