// Tests for ACP Tool Manager and Tool Calls
import { jest } from '@jest/globals';
import { ACPToolCall, ACPToolManager } from './tools.js';
import { ToolCallStatus, ToolCallKind } from './types.js';

describe('ACPToolCall', () => {
  test('should create tool call with correct initial state', () => {
    const toolCall = new ACPToolCall(
      'test-id', 
      'search', 
      ToolCallKind.search, 
      { query: 'test' }, 
      'session-123'
    );
    
    expect(toolCall.id).toBe('test-id');
    expect(toolCall.name).toBe('search');
    expect(toolCall.kind).toBe(ToolCallKind.search);
    expect(toolCall.params).toEqual({ query: 'test' });
    expect(toolCall.sessionId).toBe('session-123');
    expect(toolCall.status).toBe(ToolCallStatus.PENDING);
    expect(toolCall.startTime).toBeLessThanOrEqual(Date.now());
    expect(toolCall.endTime).toBeNull();
    expect(toolCall.result).toBeNull();
    expect(toolCall.error).toBeNull();
  });
  
  test('should update status correctly', async () => {
    const toolCall = new ACPToolCall('id', 'test', 'kind', {}, 'session');
    const result = { data: 'test result' };
    
    toolCall.updateStatus(ToolCallStatus.IN_PROGRESS);
    expect(toolCall.status).toBe(ToolCallStatus.IN_PROGRESS);
    expect(toolCall.endTime).toBeNull();
    
    // Add a small delay to ensure timing difference
    await new Promise(resolve => setTimeout(resolve, 1));
    
    toolCall.updateStatus(ToolCallStatus.COMPLETED, result);
    expect(toolCall.status).toBe(ToolCallStatus.COMPLETED);
    expect(toolCall.result).toBe(result);
    expect(toolCall.endTime).toBeGreaterThanOrEqual(toolCall.startTime);
  });
  
  test('should calculate duration correctly', (done) => {
    const toolCall = new ACPToolCall('id', 'test', 'kind', {}, 'session');
    
    setTimeout(() => {
      const duration = toolCall.getDuration();
      expect(duration).toBeGreaterThan(0);
      expect(duration).toBeLessThan(100); // Should be very small
      done();
    }, 10);
  });
  
  test('should serialize to JSON correctly', () => {
    const toolCall = new ACPToolCall(
      'test-id', 
      'search', 
      ToolCallKind.search, 
      { query: 'test' }, 
      'session-123'
    );
    toolCall.updateStatus(ToolCallStatus.COMPLETED, { found: 5 });
    
    const json = toolCall.toJSON();
    
    expect(json).toEqual({
      id: 'test-id',
      name: 'search',
      kind: ToolCallKind.search,
      params: { query: 'test' },
      sessionId: 'session-123',
      status: ToolCallStatus.COMPLETED,
      startTime: toolCall.startTime,
      endTime: toolCall.endTime,
      duration: toolCall.getDuration(),
      result: { found: 5 },
      error: null
    });
  });
});

describe('ACPToolManager', () => {
  let mockServer, mockProbeAgent, toolManager;
  
  beforeEach(() => {
    mockServer = {
      options: { debug: true },
      sendToolCallProgress: jest.fn()
    };
    
    mockProbeAgent = {
      sessionId: 'test-session',
      wrappedTools: {
        searchToolInstance: {
          execute: jest.fn().mockResolvedValue('search result')
        },
        queryToolInstance: {
          execute: jest.fn().mockResolvedValue('query result')
        },
        extractToolInstance: {
          execute: jest.fn().mockResolvedValue('extract result')
        }
      }
    };
    
    toolManager = new ACPToolManager(mockServer, mockProbeAgent);
  });
  
  describe('tool kind mapping', () => {
    test('should map tool names to correct kinds', () => {
      expect(toolManager.getToolKind('search')).toBe(ToolCallKind.search);
      expect(toolManager.getToolKind('query')).toBe(ToolCallKind.query);
      expect(toolManager.getToolKind('extract')).toBe(ToolCallKind.extract);
      expect(toolManager.getToolKind('implement')).toBe(ToolCallKind.edit);
      expect(toolManager.getToolKind('unknown')).toBe(ToolCallKind.execute);
    });
  });
  
  describe('tool execution', () => {
    test('should execute search tool successfully', async () => {
      const params = { query: 'test search', path: '/test' };
      
      const result = await toolManager.executeToolCall('session-123', 'search', params);
      
      expect(result).toBe('search result');
      expect(mockProbeAgent.wrappedTools.searchToolInstance.execute).toHaveBeenCalledWith({
        ...params,
        sessionId: 'test-session'
      });
      
      // Should send progress notifications
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledTimes(3);
      expect(mockServer.sendToolCallProgress).toHaveBeenNthCalledWith(
        1, 'session-123', expect.any(String), ToolCallStatus.PENDING
      );
      expect(mockServer.sendToolCallProgress).toHaveBeenNthCalledWith(
        2, 'session-123', expect.any(String), ToolCallStatus.IN_PROGRESS
      );
      expect(mockServer.sendToolCallProgress).toHaveBeenNthCalledWith(
        3, 'session-123', expect.any(String), ToolCallStatus.COMPLETED, 'search result'
      );
    });
    
    test('should execute query tool successfully', async () => {
      const params = { pattern: 'fn $NAME($$$)', language: 'rust' };
      
      const result = await toolManager.executeToolCall('session-123', 'query', params);
      
      expect(result).toBe('query result');
      expect(mockProbeAgent.wrappedTools.queryToolInstance.execute).toHaveBeenCalledWith({
        ...params,
        sessionId: 'test-session'
      });
    });
    
    test('should execute extract tool successfully', async () => {
      const params = { files: ['src/main.rs:10'], context_lines: 5 };
      
      const result = await toolManager.executeToolCall('session-123', 'extract', params);
      
      expect(result).toBe('extract result');
      expect(mockProbeAgent.wrappedTools.extractToolInstance.execute).toHaveBeenCalledWith({
        ...params,
        sessionId: 'test-session'
      });
    });
    
    test('should handle tool execution errors', async () => {
      const error = new Error('Tool execution failed');
      mockProbeAgent.wrappedTools.searchToolInstance.execute.mockRejectedValue(error);
      
      await expect(toolManager.executeToolCall('session-123', 'search', {})).rejects.toThrow(error);
      
      // Should send error notification
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'session-123', expect.any(String), ToolCallStatus.FAILED, null, 'Tool execution failed'
      );
    });
    
    test('should handle unknown tools', async () => {
      await expect(toolManager.executeToolCall('session-123', 'unknown', {})).rejects.toThrow('Unknown tool: unknown');
    });
    
    test('should handle missing tool instances', async () => {
      mockProbeAgent.wrappedTools.searchToolInstance = null;
      
      await expect(toolManager.executeToolCall('session-123', 'search', {})).rejects.toThrow('Search tool not available');
    });
  });
  
  describe('tool call tracking', () => {
    test('should track active tool calls', async () => {
      expect(toolManager.activeCalls.size).toBe(0);
      
      const promise = toolManager.executeToolCall('session-123', 'search', { query: 'test' });
      
      // Should have active call during execution
      expect(toolManager.activeCalls.size).toBe(1);
      
      await promise;
      
      // Should still have the call (cleaned up after timeout)
      expect(toolManager.activeCalls.size).toBe(1);
    });
    
    test('should get tool call status', async () => {
      const promise = toolManager.executeToolCall('session-123', 'search', { query: 'test' });
      
      // Get the tool call ID from the active calls
      const toolCallId = Array.from(toolManager.activeCalls.keys())[0];
      
      const status = toolManager.getToolCallStatus(toolCallId);
      expect(status).toBeDefined();
      expect(status.name).toBe('search');
      expect(status.sessionId).toBe('session-123');
      
      await promise;
      
      const completedStatus = toolManager.getToolCallStatus(toolCallId);
      expect(completedStatus.status).toBe(ToolCallStatus.COMPLETED);
    });
    
    test('should get active tool calls for session', async () => {
      await toolManager.executeToolCall('session-123', 'search', { query: 'test1' });
      await toolManager.executeToolCall('session-123', 'query', { pattern: 'test' });
      await toolManager.executeToolCall('session-456', 'extract', { files: ['test.rs'] });
      
      const session123Calls = toolManager.getActiveToolCalls('session-123');
      const session456Calls = toolManager.getActiveToolCalls('session-456');
      
      expect(session123Calls).toHaveLength(2);
      expect(session456Calls).toHaveLength(1);
      
      expect(session123Calls[0].name).toBe('search');
      expect(session123Calls[1].name).toBe('query');
      expect(session456Calls[0].name).toBe('extract');
    });
    
    test('should cancel session tool calls', () => {
      // Start some tool calls without awaiting
      toolManager.executeToolCall('session-123', 'search', { query: 'test1' });
      toolManager.executeToolCall('session-123', 'query', { pattern: 'test' });
      toolManager.executeToolCall('session-456', 'extract', { files: ['test.rs'] });
      
      // Cancel session 123 calls
      toolManager.cancelSessionToolCalls('session-123');
      
      // Should send cancellation notifications
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'session-123', expect.any(String), ToolCallStatus.FAILED, null, 'Cancelled'
      );
      
      // Session 456 calls should not be affected
      const session456Calls = toolManager.getActiveToolCalls('session-456');
      expect(session456Calls).toHaveLength(1);
      expect(session456Calls[0].status).not.toBe(ToolCallStatus.FAILED);
    });
  });
  
  describe('tool definitions', () => {
    test('should provide correct tool definitions', () => {
      const definitions = ACPToolManager.getToolDefinitions();
      
      expect(definitions).toHaveLength(3);
      
      const searchTool = definitions.find(d => d.name === 'search');
      expect(searchTool).toBeDefined();
      expect(searchTool.kind).toBe(ToolCallKind.search);
      expect(searchTool.parameters.properties.query).toBeDefined();
      expect(searchTool.parameters.required).toContain('query');
      
      const queryTool = definitions.find(d => d.name === 'query');
      expect(queryTool).toBeDefined();
      expect(queryTool.kind).toBe(ToolCallKind.query);
      expect(queryTool.parameters.properties.pattern).toBeDefined();
      expect(queryTool.parameters.required).toContain('pattern');
      
      const extractTool = definitions.find(d => d.name === 'extract');
      expect(extractTool).toBeDefined();
      expect(extractTool.kind).toBe(ToolCallKind.extract);
      expect(extractTool.parameters.properties.files).toBeDefined();
      expect(extractTool.parameters.required).toContain('files');
    });
  });
  
  describe('cleanup', () => {
    beforeEach(() => {
      jest.useFakeTimers();
    });
    
    afterEach(() => {
      jest.useRealTimers();
    });
    
    test('should clean up completed tool calls after timeout', async () => {
      await toolManager.executeToolCall('session-123', 'search', { query: 'test' });
      
      expect(toolManager.activeCalls.size).toBe(1);
      
      // Fast-forward time by 30 seconds
      jest.advanceTimersByTime(30000);
      
      expect(toolManager.activeCalls.size).toBe(0);
    });
  });
});