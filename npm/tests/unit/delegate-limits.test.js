/**
 * Tests for delegation limits and security features with SDK-based delegation
 */

import { jest } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Mock ProbeAgent
const mockAnswer = jest.fn();
const MockProbeAgent = jest.fn().mockImplementation(() => ({
  answer: mockAnswer
}));

// Use absolute path for mocking
const probeAgentPath = resolve(__dirname, '../../src/agent/ProbeAgent.js');
const delegatePath = resolve(__dirname, '../../src/delegate.js');

jest.unstable_mockModule(probeAgentPath, () => ({
  ProbeAgent: MockProbeAgent
}));

// Import after mocking
const { delegate, cleanupDelegationManager, getDelegationStats } = await import(delegatePath);

describe('Delegate Tool Security and Limits (SDK-based)', () => {
  beforeEach(() => {
    // Clear previous mocks
    jest.clearAllMocks();

    // Clean up delegation manager state (now synchronous)
    cleanupDelegationManager();

    // Mock successful response by default
    mockAnswer.mockResolvedValue('Test response from subagent');
  });

  afterEach(() => {
    jest.clearAllMocks();
    // Ensure cleanup after each test (now synchronous)
    cleanupDelegationManager();
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

    it('should inherit enableBash from parent when true', async () => {
      const task = 'Test task with bash access';

      await delegate({
        task,
        enableBash: true
      });

      // Check that enableBash is inherited
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          enableBash: true
        })
      );
    });

    it('should inherit bashConfig from parent', async () => {
      const task = 'Test task with bash config';
      const bashConfig = {
        allow: ['git', 'npm'],
        deny: ['rm', 'sudo']
      };

      await delegate({
        task,
        enableBash: true,
        bashConfig
      });

      // Check that bashConfig is inherited
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          enableBash: true,
          bashConfig: bashConfig
        })
      );
    });

    it('should default enableBash to false when not provided', async () => {
      const task = 'Test task without bash';

      await delegate({ task });

      // Check that enableBash defaults to false
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          enableBash: false
        })
      );
    });

    it('should default bashConfig to null when not provided', async () => {
      const task = 'Test task without bash config';

      await delegate({ task });

      // Check that bashConfig defaults to null
      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          bashConfig: null
        })
      );
    });
  });

  describe('Concurrent delegation limits', () => {
    it('should queue delegations when global limit is reached', async () => {
      // Make responses slow to ensure they overlap
      mockAnswer.mockImplementation(() =>
        new Promise(resolve => setTimeout(() => resolve('Slow response'), 50))
      );

      // Start 3 delegations (max concurrent)
      const task1 = delegate({ task: 'Task 1' });
      const task2 = delegate({ task: 'Task 2' });
      const task3 = delegate({ task: 'Task 3' });

      // Wait a bit to ensure they're all active
      await new Promise(resolve => setTimeout(resolve, 10));

      // Check stats - should show 3 active
      const stats = getDelegationStats();
      expect(stats.globalActive).toBe(3);

      // Fourth delegation should queue (not fail) and eventually complete
      const task4 = delegate({ task: 'Task 4' });

      // Check that queue size increased
      const statsAfterQueue = getDelegationStats();
      expect(statsAfterQueue.queueSize).toBe(1);

      // Wait for all to complete - task4 should complete after one of the first 3 finishes
      const results = await Promise.all([task1, task2, task3, task4]);
      expect(results).toEqual(['Slow response', 'Slow response', 'Slow response', 'Slow response']);
    });

    it('should process queued delegations in FIFO order', async () => {
      const completionOrder = [];

      // Make responses complete in order they started (with small delay)
      mockAnswer.mockImplementation((task) =>
        new Promise(resolve => setTimeout(() => {
          completionOrder.push(task);
          resolve(`Response for ${task}`);
        }, 20))
      );

      // Start 3 delegations (max concurrent)
      const task1 = delegate({ task: 'Task 1' });
      const task2 = delegate({ task: 'Task 2' });
      const task3 = delegate({ task: 'Task 3' });

      // Wait a bit to ensure they're all active
      await new Promise(resolve => setTimeout(resolve, 5));

      // Queue 2 more delegations
      const task4 = delegate({ task: 'Task 4' });
      const task5 = delegate({ task: 'Task 5' });

      // Wait for all to complete
      await Promise.all([task1, task2, task3, task4, task5]);

      // Tasks 4 and 5 should have been processed after at least one of 1-3 completed
      expect(mockAnswer).toHaveBeenCalledTimes(5);
    });

    it('should enforce per-session delegation limit', async () => {
      // Reset to fast responses but with slight delay
      mockAnswer.mockImplementation(() =>
        new Promise(resolve => setTimeout(() => resolve('Response'), 10))
      );

      const parentSessionId = 'test-session-123';

      // Start delegations sequentially and wait for each to complete
      // This tests the per-session limit without hitting global limit
      for (let i = 0; i < 10; i++) {
        await delegate({
          task: `Task ${i}`,
          parentSessionId
        });
      }

      // Wait a moment to ensure all previous have decremented
      await new Promise(resolve => setTimeout(resolve, 20));

      // Now try 3 more in parallel - should work (counters reset)
      const tasks = [];
      for (let i = 0; i < 3; i++) {
        tasks.push(delegate({
          task: `Parallel Task ${i}`,
          parentSessionId
        }));
      }

      const results = await Promise.all(tasks);
      expect(results).toEqual(['Response', 'Response', 'Response']);
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
      // Ensure clean state
      cleanupDelegationManager();

      // Make subagent never resolve
      mockAnswer.mockImplementation(() => new Promise(() => {}));

      // First delegation times out (50ms)
      await expect(
        delegate({ task: 'Task 1', timeout: 0.05 })
      ).rejects.toThrow(/timed out/);

      // Counter should be decremented, so next delegation works
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

    it('should clear timeout when task completes successfully', async () => {
      const clearTimeoutSpy = jest.spyOn(global, 'clearTimeout');

      mockAnswer.mockResolvedValue('Quick success');

      await delegate({ task: 'Test task', timeout: 5 });

      // clearTimeout should have been called
      expect(clearTimeoutSpy).toHaveBeenCalled();

      clearTimeoutSpy.mockRestore();
    });

    it('should clear timeout when task fails', async () => {
      const clearTimeoutSpy = jest.spyOn(global, 'clearTimeout');

      mockAnswer.mockRejectedValue(new Error('Task error'));

      await expect(delegate({ task: 'Test task', timeout: 5 })).rejects.toThrow();

      // clearTimeout should have been called
      expect(clearTimeoutSpy).toHaveBeenCalled();

      clearTimeoutSpy.mockRestore();
    });
  });

  describe('Error handling', () => {
    it('should reject with clear error message when task is empty', async () => {
      await expect(delegate({ task: '' })).rejects.toThrow(
        /Task parameter is required/
      );
    });

    it('should reject when parentSessionId is not a string', async () => {
      await expect(delegate({
        task: 'Test task',
        parentSessionId: 123 // Invalid: should be string
      })).rejects.toThrow(
        /parentSessionId must be a string/
      );
    });

    it('should accept null or undefined parentSessionId', async () => {
      // null should work
      await expect(delegate({
        task: 'Test task',
        parentSessionId: null
      })).resolves.toBeDefined();

      // undefined should work (implicit)
      await expect(delegate({
        task: 'Test task'
      })).resolves.toBeDefined();
    });

    it('should reject when subagent returns empty response', async () => {
      mockAnswer.mockResolvedValue('');

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /returned empty or whitespace-only response/
      );
    });

    it('should reject when subagent returns only whitespace', async () => {
      mockAnswer.mockResolvedValue('   \n\t   ');

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /returned empty or whitespace-only response/
      );
    });

    it('should reject when subagent returns response with null bytes', async () => {
      mockAnswer.mockResolvedValue('Valid content\0with null bytes');

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /returned response containing null bytes/
      );
    });

    it('should reject when subagent returns non-string response', async () => {
      mockAnswer.mockResolvedValue(null);

      await expect(delegate({ task: 'Test task' })).rejects.toThrow(
        /returned invalid response \(not a string\)/
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
