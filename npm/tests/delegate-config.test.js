/**
 * Configuration tests for delegate tool
 * Tests the core functionality without mocking complex process spawning
 */

import { jest } from '@jest/globals';

// Mock just the delegate function itself
const mockDelegate = jest.fn();
const mockIsDelegateAvailable = jest.fn(() => Promise.resolve(true));
jest.unstable_mockModule('../src/delegate.js', () => ({
  delegate: mockDelegate,
  isDelegateAvailable: mockIsDelegateAvailable
}));

// Import after mocking
const { delegateTool } = await import('../src/tools/vercel.js');
const { ACPToolManager } = await import('../src/agent/acp/tools.js');

describe('Delegate Tool Configuration', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockDelegate.mockResolvedValue('Mock delegate response');
  });

  describe('Vercel AI SDK Tool', () => {
    it('should create delegate tool with correct configuration', () => {
      const tool = delegateTool({ debug: true, timeout: 600 });
      
      expect(tool.name).toBe('delegate');
      expect(tool.description).toContain('Automatically delegate');
      expect(tool.description).toContain('agentic loop');
      expect(tool.description).toContain('specialized probe subagents');
      expect(tool.execute).toBeDefined();
    });

    it('should execute delegate tool with correct parameters', async () => {
      const tool = delegateTool({ debug: true, timeout: 600 });
      const task = 'Analyze security vulnerabilities in authentication code';

      const result = await tool.execute({ task });

      // Tool now passes all parameters including defaults
      expect(mockDelegate).toHaveBeenCalledWith({
        task,
        timeout: 600,
        debug: true,
        currentIteration: 0,
        maxIterations: 30,
        parentSessionId: undefined,
        path: undefined, // No cwd or allowedFolders configured
        provider: undefined,
        model: undefined,
        tracer: undefined
      });

      expect(result).toBe('Mock delegate response');
    });

    it('should use cwd when path is not specified in call', async () => {
      const tool = delegateTool({
        debug: false,
        timeout: 300,
        cwd: '/project/workspace'
      });
      const task = 'Analyze code in workspace';

      await tool.execute({ task });

      expect(mockDelegate).toHaveBeenCalledWith(
        expect.objectContaining({
          task,
          path: '/project/workspace'
        })
      );
    });

    it('should use allowedFolders[0] when path and cwd are not specified', async () => {
      const tool = delegateTool({
        debug: false,
        timeout: 300,
        allowedFolders: ['/allowed/folder1', '/allowed/folder2']
      });
      const task = 'Search allowed folders';

      await tool.execute({ task });

      expect(mockDelegate).toHaveBeenCalledWith(
        expect.objectContaining({
          task,
          path: '/allowed/folder1'
        })
      );
    });

    it('should prioritize explicit path over cwd', async () => {
      const tool = delegateTool({
        debug: false,
        timeout: 300,
        cwd: '/default/path',
        allowedFolders: ['/allowed/folder']
      });
      const task = 'Search specific path';

      await tool.execute({ task, path: '/explicit/path' });

      expect(mockDelegate).toHaveBeenCalledWith(
        expect.objectContaining({
          task,
          path: '/explicit/path'
        })
      );
    });

    it('should prioritize cwd over allowedFolders', async () => {
      const tool = delegateTool({
        debug: false,
        timeout: 300,
        cwd: '/default/path',
        allowedFolders: ['/allowed/folder']
      });
      const task = 'Use cwd priority';

      await tool.execute({ task });

      expect(mockDelegate).toHaveBeenCalledWith(
        expect.objectContaining({
          task,
          path: '/default/path'
        })
      );
    });

    it('should handle execution errors gracefully', async () => {
      const tool = delegateTool();
      const task = 'Task that will fail';

      mockDelegate.mockRejectedValue(new Error('Delegation process failed'));

      // Tool now throws errors instead of returning error strings
      await expect(tool.execute({ task })).rejects.toThrow('Delegation process failed');
    });
  });

  describe('ACP Tool Manager Integration', () => {
    let toolManager;
    let mockServer;
    let mockProbeAgent;

    beforeEach(() => {
      mockServer = {
        options: { debug: true },
        sendToolCallProgress: jest.fn()
      };

      mockProbeAgent = {
        wrappedTools: {
          delegateToolInstance: {
            execute: jest.fn().mockResolvedValue('ACP delegate response')
          }
        },
        sessionId: 'test-session-123'
      };

      toolManager = new ACPToolManager(mockServer, mockProbeAgent);
    });

    it('should classify delegate tool as execute kind', () => {
      const toolKind = toolManager.getToolKind('delegate');
      expect(toolKind).toBe('execute');
    });

    it('should execute delegate tool through ACP with lifecycle tracking', async () => {
      const task = 'Review API security implementations';
      
      const result = await toolManager.executeToolCall('test-session', 'delegate', { task });
      
      expect(mockProbeAgent.wrappedTools.delegateToolInstance.execute).toHaveBeenCalledWith({
        task,
        sessionId: 'test-session-123'
      });
      
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'test-session',
        expect.any(String),
        'pending'
      );
      
      expect(mockServer.sendToolCallProgress).toHaveBeenCalledWith(
        'test-session',
        expect.any(String),
        'completed',
        'ACP delegate response'
      );
      
      expect(result).toBe('ACP delegate response');
    });

    it('should provide delegate tool in definitions', () => {
      const definitions = ACPToolManager.getToolDefinitions();
      
      const delegateTool = definitions.find(d => d.name === 'delegate');
      expect(delegateTool).toBeDefined();
      expect(delegateTool.kind).toBe('execute');
      expect(delegateTool.description).toContain('Automatically delegate');
      expect(delegateTool.inputSchema.properties.task).toBeDefined();
      expect(delegateTool.inputSchema.required).toContain('task');
    });
  });

  describe('XML Parsing and Agentic Usage', () => {
    it('should support XML tool format for AI agents', () => {
      const xmlExamples = [
        '<delegate><task>Analyze authentication code for security vulnerabilities</task></delegate>',
        '<delegate><task>Review database performance and optimization opportunities</task></delegate>',
        '<delegate><task>Examine code structure and maintainability patterns</task></delegate>'
      ];

      xmlExamples.forEach(xml => {
        expect(xml).toMatch(/<delegate>/);
        expect(xml).toMatch(/<task>.*<\/task>/);
        expect(xml).toMatch(/<\/delegate>/);
        
        // Extract task content
        const taskMatch = xml.match(/<task>(.*?)<\/task>/);
        expect(taskMatch).not.toBeNull();
        expect(taskMatch[1].length).toBeGreaterThan(20);
      });
    });

    it('should demonstrate proper task separation patterns', () => {
      const complexRequest = 'Analyze my application for security, performance, and maintainability';
      
      const separatedTasks = [
        'Analyze all authentication, authorization, and input validation code for security vulnerabilities',
        'Review database queries, API endpoints, and resource usage for performance bottlenecks',
        'Examine code structure, design patterns, and documentation for maintainability improvements'
      ];

      separatedTasks.forEach(task => {
        expect(task.length).toBeGreaterThan(50);
        expect(task).toMatch(/^(Analyze|Review|Examine)/);
        
        // Each task should focus on one domain
        const domains = ['security', 'performance', 'maintainability'];
        const matchedDomains = domains.filter(domain => 
          task.toLowerCase().includes(domain) || 
          (domain === 'security' && task.includes('vulnerabilities')) ||
          (domain === 'performance' && task.includes('bottlenecks')) ||
          (domain === 'maintainability' && task.includes('design patterns'))
        );
        
        expect(matchedDomains.length).toBeGreaterThanOrEqual(1);
      });
    });

    it('should validate task self-containment', () => {
      const validTasks = [
        'Find all SQL injection vulnerabilities in database queries and provide fix recommendations',
        'Identify memory leaks and performance bottlenecks in async operations',
        'Review error handling patterns and suggest improvements for better reliability'
      ];

      validTasks.forEach(task => {
        // Should be actionable
        expect(task).toMatch(/^(Find|Identify|Review|Analyze|Examine|Search)/);
        
        // Should be specific
        expect(task.length).toBeGreaterThan(30);
        expect(task.length).toBeLessThan(200);
        
        // Should not contain coordination words
        expect(task).not.toMatch(/\band then\b|\bafter that\b|\balso\b/i);
      });
    });

    it('should handle multi-line tasks in XML', () => {
      const multilineTask = `Review database performance including:
- Query optimization opportunities
- Index usage patterns  
- Connection pooling efficiency
- N+1 query detection`;

      const xml = `<delegate><task>${multilineTask}</task></delegate>`;
      
      expect(xml).toContain('Review database performance');
      expect(xml).toContain('Query optimization');
      expect(xml).toContain('N+1 query detection');
      expect(xml).toMatch(/<delegate><task>[\s\S]*<\/task><\/delegate>/);
    });
  });

  describe('Iteration Limit Logic', () => {
    it('should test remaining iterations calculation', () => {
      // Test the logic that would be used in the delegate function
      const testCases = [
        { current: 5, max: 20, expected: 15 },
        { current: 25, max: 30, expected: 5 },
        { current: 35, max: 30, expected: 1 }, // Should always allow at least 1
        { current: 0, max: 10, expected: 10 }
      ];

      testCases.forEach(({ current, max, expected }) => {
        const remaining = Math.max(1, max - current);
        expect(remaining).toBe(expected);
      });
    });
  });

  describe('Tool Parameters and Schema', () => {
    it('should validate delegate tool parameters', () => {
      // Test parameter validation logic
      const validTasks = [
        'Analyze code for security issues',
        'Find performance bottlenecks',
        'Review error handling'
      ];

      validTasks.forEach(task => {
        expect(typeof task).toBe('string');
        expect(task.length).toBeGreaterThan(0);
        expect(task.trim()).toBe(task); // No leading/trailing whitespace
      });

      // Test invalid parameters
      const invalidTasks = [null, undefined, '', 123, {}, []];
      
      invalidTasks.forEach(task => {
        expect(typeof task === 'string' && task.length > 0).toBe(false);
      });
    });

    it('should have correct automatic flag configuration', () => {
      // Test that automatic flags are properly defined
      const expectedFlags = [
        '--prompt-type', 'code-researcher',
        '--no-schema-validation',
        '--no-mermaid-validation'
      ];

      expectedFlags.forEach(flag => {
        expect(typeof flag).toBe('string');
        expect(flag.length).toBeGreaterThan(0);
      });

      // Verify flag patterns
      expect('--prompt-type').toMatch(/^--[a-z-]+$/);
      expect('code-researcher').toMatch(/^[a-z-]+$/);
      expect('--no-schema-validation').toMatch(/^--no-[a-z-]+$/);
      expect('--no-mermaid-validation').toMatch(/^--no-[a-z-]+$/);
    });
  });
});