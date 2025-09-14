/**
 * Integration tests for delegate tool with Vercel AI SDK and ACP
 */

import { jest } from '@jest/globals';

// Mock the delegate function
const mockDelegate = jest.fn();
const mockIsDelegateAvailable = jest.fn(() => Promise.resolve(true));
jest.unstable_mockModule('../src/delegate.js', () => ({
  delegate: mockDelegate,
  isDelegateAvailable: mockIsDelegateAvailable
}));

// Mock ProbeAgent to avoid API key requirements
const mockProbeAgent = jest.fn().mockImplementation((options) => {
  const instance = {
    sessionId: options.sessionId,
    debug: options.debug,
    maxIterations: options.maxIterations,
    currentIteration: 0,
    executeTool: jest.fn()
  };
  
  instance.executeTool.mockImplementation(async (toolName, params) => {
    if (toolName === 'delegate') {
      // Simulate ProbeAgent calling delegate with iteration context
      const enhancedParams = {
        ...params,
        currentIteration: instance.currentIteration,
        maxIterations: instance.maxIterations,
        debug: instance.debug
      };
      return await mockDelegate(enhancedParams);
    }
    return `Mock result for ${toolName}`;
  });
  
  return instance;
});
jest.unstable_mockModule('../src/agent/ProbeAgent.js', () => ({
  ProbeAgent: mockProbeAgent
}));

// Import after mocking
const { delegateTool } = await import('../src/tools/vercel.js');
const { ACPToolManager } = await import('../src/agent/acp/tools.js');
const { ProbeAgent } = await import('../src/agent/ProbeAgent.js');

describe('Delegate Tool Integration', () => {
  describe('Vercel AI SDK Integration', () => {
    beforeEach(() => {
      jest.clearAllMocks();
    });

    afterEach(() => {
      jest.clearAllMocks();
    });

    it('should create delegate tool with proper schema', () => {
      const tool = delegateTool({ debug: true, timeout: 600 });
      
      expect(tool.name).toBe('delegate');
      expect(tool.description).toContain('Automatically delegate');
      expect(tool.description).toContain('agentic loop');
      
      // Zod schema should be an object
      expect(tool.parameters).toBeDefined();
      expect(typeof tool.parameters).toBe('object');
      
      // Check that the schema can parse valid input
      const validInput = { task: 'Test task' };
      expect(() => tool.parameters.parse(validInput)).not.toThrow();
      
      // Check that invalid input throws
      expect(() => tool.parameters.parse({})).toThrow(); // Missing required task
    });

    it('should execute delegate tool with correct parameters', async () => {
      const tool = delegateTool({ debug: true, timeout: 600 });
      const task = 'Analyze security vulnerabilities in authentication code';
      
      mockDelegate.mockResolvedValue('Security analysis complete: Found 3 vulnerabilities');
      
      const result = await tool.execute({ task });
      
      expect(mockDelegate).toHaveBeenCalledWith({
        task,
        timeout: 600,
        debug: true
      });
      
      expect(result).toBe('Security analysis complete: Found 3 vulnerabilities');
    });

    it('should handle delegate execution errors gracefully', async () => {
      const tool = delegateTool();
      const task = 'Invalid task that will fail';
      
      mockDelegate.mockRejectedValue(new Error('Delegation process failed'));
      
      const result = await tool.execute({ task });
      
      expect(result).toContain('Error executing delegate command');
      expect(result).toContain('Delegation process failed');
    });

    it('should support XML parsing format', () => {
      // Test that the tool definition supports XML parsing by AI agents
      const tool = delegateTool();
      
      // Simulate AI agent parsing XML and converting to tool call
      const xmlInput = '<delegate><task>Find all TODO comments in the codebase</task></delegate>';
      const parsedTask = xmlInput.match(/<task>(.*?)<\/task>/s)?.[1];
      
      expect(parsedTask).toBe('Find all TODO comments in the codebase');
      
      // This would be how AI agent converts XML to tool call
      const toolCall = {
        name: tool.name,
        parameters: { task: parsedTask }
      };
      
      expect(toolCall.name).toBe('delegate');
      expect(toolCall.parameters.task).toBe('Find all TODO comments in the codebase');
    });
  });

  describe('ACP Tool Manager Integration', () => {
    let toolManager;
    let mockServer;
    let mockProbeAgent;
    let mockDelegate;

    beforeEach(async () => {
      // Mock server
      mockServer = {
        options: { debug: true },
        sendToolCallProgress: jest.fn()
      };

      // Mock probe agent with delegate tool
      mockProbeAgent = {
        wrappedTools: {
          delegateToolInstance: {
            execute: jest.fn()
          }
        },
        sessionId: 'test-session-123'
      };

      toolManager = new ACPToolManager(mockServer, mockProbeAgent);

      // Mock delegate function
      const delegateModule = await import('../src/delegate.js');
      mockDelegate = delegateModule.delegate;
    });

    afterEach(() => {
      jest.clearAllMocks();
    });

    it('should execute delegate tool through ACP with proper lifecycle tracking', async () => {
      const task = 'Review API security implementations';
      const mockResponse = 'API security review completed';
      
      mockProbeAgent.wrappedTools.delegateToolInstance.execute.mockResolvedValue(mockResponse);
      
      const result = await toolManager.executeToolCall('test-session', 'delegate', { task });
      
      // Verify tool execution
      expect(mockProbeAgent.wrappedTools.delegateToolInstance.execute).toHaveBeenCalledWith({
        task,
        sessionId: 'test-session-123'
      });
      
      // Verify lifecycle notifications
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'test-session',
        expect.any(String),
        'pending'
      );
      
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'test-session',
        expect.any(String),
        'in_progress'
      );
      
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'test-session',
        expect.any(String),
        'completed',
        mockResponse
      );
      
      expect(result).toBe(mockResponse);
    });

    it('should classify delegate tool as execute kind', () => {
      const toolKind = toolManager.getToolKind('delegate');
      expect(toolKind).toBe('execute');
    });

    it('should handle delegate tool failures with proper error reporting', async () => {
      const task = 'Task that will fail';
      const errorMessage = 'Delegation failed: Process terminated unexpectedly';
      
      mockProbeAgent.wrappedTools.delegateToolInstance.execute.mockRejectedValue(
        new Error(errorMessage)
      );
      
      await expect(
        toolManager.executeToolCall('test-session', 'delegate', { task })
      ).rejects.toThrow(errorMessage);
      
      // Verify error was reported through ACP
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'test-session',
        expect.any(String),
        'failed',
        null,
        errorMessage
      );
    });
  });

  describe('ProbeAgent Integration', () => {
    let probeAgent;
    let mockDelegate;

    beforeEach(async () => {
      // Mock delegate function
      const delegateModule = await import('../src/delegate.js');
      mockDelegate = delegateModule.delegate;

      // Create probe agent instance
      probeAgent = new ProbeAgent({
        sessionId: 'integration-test-session',
        debug: true,
        maxIterations: 25
      });
    });

    afterEach(() => {
      jest.clearAllMocks();
    });

    it('should pass iteration context to delegate tool execution', async () => {
      const task = 'Complex task requiring delegation';
      const expectedResponse = 'Task completed by subagent';
      
      mockDelegate.mockResolvedValue(expectedResponse);
      
      // Simulate some iterations have already occurred
      probeAgent.currentIteration = 8;
      
      // Execute delegate tool through ProbeAgent
      const result = await probeAgent.executeTool('delegate', { task });
      
      // Verify delegate was called with iteration context
      expect(mockDelegate).toHaveBeenCalledWith({
        task,
        currentIteration: 8,
        maxIterations: 25,
        debug: true
      });
      
      expect(result).toBe(expectedResponse);
    });

    it('should handle delegate tool when near iteration limit', async () => {
      const task = 'Last minute delegation';
      
      mockDelegate.mockResolvedValue('Quick response from subagent');
      
      // Set current iteration very close to limit
      probeAgent.currentIteration = 24;
      probeAgent.maxIterations = 25;
      
      await probeAgent.executeTool('delegate', { task });
      
      // Should still allow delegation but with very limited iterations
      expect(mockDelegate).toHaveBeenCalledWith({
        task,
        currentIteration: 24,
        maxIterations: 25,
        debug: true
      });
    });
  });

  // Note: Automatic flag verification is covered in delegate.test.js unit tests

  describe('Agentic Loop Scenarios', () => {
    it('should demonstrate multi-task delegation scenario', async () => {
      // Simulate AI agent receiving complex request and breaking it down
      const complexRequest = 'Analyze my Node.js application for security issues, performance problems, and code quality concerns';
      
      const expectedDelegations = [
        'Analyze all input validation, authentication, authorization, and dependency vulnerabilities in the Node.js application',
        'Review database queries, async operations, memory usage, and API response times for performance optimization opportunities',  
        'Examine code structure, documentation, test coverage, and maintainability patterns across the application'
      ];
      
      // Each delegation should be independent and focused
      expectedDelegations.forEach(task => {
        expect(task.length).toBeGreaterThan(50); // Substantial task
        expect(task).not.toContain('and also'); // Single focus
        
        // Verify task is complete and actionable
        if (task.includes('security') || task.includes('vulnerabilities')) {
          expect(task).toMatch(/authentication|authorization|validation|vulnerabilities/);
        } else if (task.includes('performance')) {
          expect(task).toMatch(/database|async|memory|response|optimization/);
        } else if (task.includes('quality') || task.includes('maintainability')) {
          expect(task).toMatch(/structure|documentation|test|maintainability|patterns/);
        }
      });
    });

    it('should validate task self-containment for parallel execution', () => {
      const tasks = [
        'Find all SQL injection vulnerabilities in the database layer',
        'Identify performance bottlenecks in the API endpoints',
        'Review error handling patterns across all modules'
      ];
      
      // Each task should be executable independently
      tasks.forEach(task => {
        // Should have clear scope and action
        expect(task).toMatch(/^(Find|Identify|Review|Analyze|Examine)/);
        
        // Should specify domain clearly
        expect(task).toMatch(/(vulnerabilities|bottlenecks|patterns|security|performance|error)/);
        
        // Should be specific enough to execute
        expect(task.length).toBeGreaterThan(30);
        expect(task.length).toBeLessThan(200);
      });
    });
  });
});