/**
 * Tests for delegation limits and security features with SDK-based delegation
 */

import { jest } from '@jest/globals';

// Mock ProbeAgent
const mockAnswer = jest.fn();
const MockProbeAgent = jest.fn().mockImplementation(() => ({
  answer: mockAnswer
}));

jest.unstable_mockModule('../src/agent/ProbeAgent.js', () => ({
  ProbeAgent: MockProbeAgent
}));

// Import after mocking
const { delegate } = await import('../src/delegate.js');

describe('Delegate Tool Security and Limits (SDK-based)', () => {
  beforeEach(() => {
    // Clear previous mocks
    jest.clearAllMocks();

    // Mock successful response by default
    mockAnswer.mockResolvedValue('Test response from subagent');
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('Recursion prevention', () => {
    it('should create ProbeAgent with enableDelegate=false to prevent recursion', async () => {
      const task = 'Test task';

      await delegate({ task });

      // Check that ProbeAgent was created with enableDelegate: false
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          enableDelegate: false
        })
      );
    });

    it('should use code-researcher prompt for subagent', async () => {
      const task = 'Test task';

      await delegate({ task });

      // Check that ProbeAgent was created with code-researcher prompt
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          promptType: 'code-researcher'
        })
      );
    });

    it('should disable validations for faster processing', async () => {
      const task = 'Test task';

      await delegate({ task });

      // Check that validations are disabled
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          disableMermaidValidation: true,
          disableJsonValidation: true
        })
      );
    });

    it('should pass remaining iterations to subagent', async () => {
      const task = 'Test task';

      await delegate({
        task,
        currentIteration: 5,
        maxIterations: 30
      });

      // Check that maxIterations is calculated correctly (30 - 5 = 25)
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          maxIterations: 25
        })
      );
    });

    it('should inherit path, provider, and model from parent', async () => {
      const task = 'Test task';

      await delegate({
        task,
        path: '/test/path',
        provider: 'anthropic',
        model: 'claude-3-opus'
      });

      // Check that config is inherited
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          path: '/test/path',
          provider: 'anthropic',
          model: 'claude-3-opus'
        })
      );
    });
  });

  describe('Concurrent delegation limits', () => {
    it('should enforce global concurrent delegation limit', async () => {
      // Start 3 delegations (max)
      const task1 = delegate({ task: 'Task 1' });
      const task2 = delegate({ task: 'Task 2' });
      const task3 = delegate({ task: 'Task 3' });

      // Fourth delegation should fail immediately
      await expect(delegate({ task: 'Task 4' })).rejects.toThrow(
        /Maximum concurrent delegations.*reached/
      );

      // Clean up
      await Promise.allSettled([task1, task2, task3]);
    });

    it('should enforce per-session delegation limit', async () => {
      const parentSessionId = 'test-session-123';

      // Start 10 delegations for the same session (max)
      const tasks = [];
      for (let i = 0; i < 10; i++) {
        tasks.push(delegate({
          task: `Task ${i}`,
          parentSessionId
        }));
      }

      // 11th delegation for same session should fail
      await expect(delegate({
        task: 'Task 11',
        parentSessionId
      })).rejects.toThrow(
        /Maximum delegations per session.*reached/
      );

      // Clean up
      await Promise.allSettled(tasks);
    });

    it('should decrement counter when delegation completes successfully', async () => {
      // Complete first delegation
      await delegate({ task: 'Task 1' });

      // Should be able to start another (counter was decremented)
      await expect(delegate({ task: 'Task 2' })).resolves.toBeDefined();
    });

    it('should decrement counter when delegation fails', async () => {
      // Make subagent fail
      mockAnswer.mockRejectedValueOnce(new Error('Subagent error'));

      // First delegation fails
      await expect(delegate({ task: 'Task 1' })).rejects.toThrow();

      // Counter should be decremented, so next delegation works
      mockAnswer.mockResolvedValueOnce('Success');
      await expect(delegate({ task: 'Task 2' })).resolves.toBeDefined();
    });

    it('should decrement counter on timeout', async () => {
      // Make subagent never resolve
      mockAnswer.mockImplementation(() => new Promise(() => {}));

      // First delegation times out (50ms)
      await expect(
        delegate({ task: 'Task 1', timeout: 0.05 })
      ).rejects.toThrow(/timed out/);

      // Counter should be decremented
      mockAnswer.mockResolvedValueOnce('Success');
      await expect(delegate({ task: 'Task 2' })).resolves.toBeDefined();
    });
  });

  describe('Parent session tracking', () => {
    it('should track delegations per parent session independently', async () => {
      const session1 = 'session-1';
      const session2 = 'session-2';

      // Start delegations for both sessions
      await delegate({ task: 'Task 1', parentSessionId: session1 });
      await delegate({ task: 'Task 2', parentSessionId: session2 });

      // Both should succeed (different sessions)
      expect(mockAnswer).toHaveBeenCalledTimes(2);
    });

    it('should clean up session tracking after delegations complete', async () => {
      const parentSessionId = 'cleanup-test';

      // Complete a delegation
      await delegate({ task: 'Task 1', parentSessionId });

      // Session should be cleaned up, so can start fresh
      await delegate({ task: 'Task 2', parentSessionId });

      expect(mockAnswer).toHaveBeenCalledTimes(2);
    });
  });

  describe('Timeout handling', () => {
    it('should timeout after specified duration', async () => {
      // Make subagent take too long
      mockAnswer.mockImplementation(() =>
        new Promise(resolve => setTimeout(() => resolve('Late'), 200))
      );

      // Should timeout at 50ms
      await expect(
        delegate({ task: 'Test task', timeout: 0.05 })
      ).rejects.toThrow(/timed out after 0.05 seconds/);
    });

    it('should complete if response comes before timeout', async () => {
      // Make subagent respond quickly
      mockAnswer.mockImplementation(() =>
        new Promise(resolve => setTimeout(() => resolve('Quick response'), 10))
      );

      // Should complete successfully with 1 second timeout
      await expect(
        delegate({ task: 'Test task', timeout: 1 })
      ).resolves.toBe('Quick response');
    });
  });

  describe('Error handling', () => {
    it('should reject with clear error message when task is empty', async () => {
      await expect(delegate({ task: '' })).rejects.toThrow(
        /Task parameter is required/
      );
    });

    it('should reject when subagent returns empty response', async () => {
      mockAnswer.mockResolvedValue('');

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /returned empty response/
      );
    });

    it('should reject when subagent returns only whitespace', async () => {
      mockAnswer.mockResolvedValue('   \n\t   ');

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /returned empty response/
      );
    });

    it('should propagate subagent errors with context', async () => {
      mockAnswer.mockRejectedValue(new Error('AI provider error'));

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /Delegation failed: AI provider error/
      );
    });
  });

  describe('SDK integration', () => {
    it('should call subagent.answer() with the task', async () => {
      const task = 'Analyze the codebase structure';

      await delegate({ task });

      expect(mockAnswer).toHaveBeenCalledWith(task);
    });

    it('should return the subagent response', async () => {
      const expectedResponse = 'Detailed analysis results';
      mockAnswer.mockResolvedValue(expectedResponse);

      const result = await delegate({ task: 'Test task' });

      expect(result).toBe(expectedResponse);
    });

    it('should create unique session ID for each delegation', async () => {
      await delegate({ task: 'Task 1' });
      await delegate({ task: 'Task 2' });

      // Check that two different session IDs were generated
      const call1SessionId = MockProbeAgent.mock.calls[0][0].sessionId;
      const call2SessionId = MockProbeAgent.mock.calls[1][0].sessionId;

      expect(call1SessionId).not.toBe(call2SessionId);
      expect(call1SessionId).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    });
  });
});
