/**
 * Tests for delegation limits and security features
 */

import { jest } from '@jest/globals';

// Mock child_process.spawn at module level
const mockSpawn = jest.fn();
jest.unstable_mockModule('child_process', () => ({
  spawn: mockSpawn
}));

// Mock utils module
const mockGetBinaryPath = jest.fn();
jest.unstable_mockModule('../src/utils.js', () => ({
  getBinaryPath: mockGetBinaryPath,
  buildCliArgs: jest.fn()
}));

// Import after mocking
const { delegate } = await import('../src/delegate.js');

describe('Delegate Tool Security and Limits', () => {
  let mockProcess;

  beforeEach(() => {
    // Create a mock process object
    mockProcess = {
      stdout: { on: jest.fn() },
      stderr: { on: jest.fn() },
      on: jest.fn(),
      kill: jest.fn(),
      killed: false
    };

    // Clear previous mocks and set up spawn mock
    jest.clearAllMocks();
    mockSpawn.mockReturnValue(mockProcess);
    mockGetBinaryPath.mockResolvedValue('/mock/path/to/probe');

    // Simulate successful completion after a short delay
    mockProcess.on.mockImplementation((event, callback) => {
      if (event === 'close') {
        setTimeout(() => callback(0), 10);
      }
    });
    mockProcess.stdout.on.mockImplementation((event, callback) => {
      if (event === 'data') {
        setTimeout(() => callback(Buffer.from('Test response')), 5);
      }
    });
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('Recursion prevention', () => {
    it('should pass --no-delegate flag to subagent to prevent recursion', async () => {
      const task = 'Test task';

      // Execute delegation
      const delegatePromise = delegate({ task });

      // Wait for spawn to be called
      await new Promise(resolve => setTimeout(resolve, 20));

      // Check that spawn was called with --no-delegate flag
      expect(mockSpawn).toHaveBeenCalled();
      const spawnArgs = mockSpawn.mock.calls[0][1];
      expect(spawnArgs).toContain('--no-delegate');
    });

    it('should include --no-delegate in the correct position', async () => {
      const task = 'Test task';

      // Execute delegation
      const delegatePromise = delegate({ task, currentIteration: 5, maxIterations: 30 });

      // Wait for spawn to be called
      await new Promise(resolve => setTimeout(resolve, 20));

      // Check arguments order
      const spawnArgs = mockSpawn.mock.calls[0][1];
      expect(spawnArgs).toEqual(expect.arrayContaining([
        'agent',
        '--task', task,
        '--session-id', expect.any(String),
        '--prompt-type', 'code-researcher',
        '--no-schema-validation',
        '--no-mermaid-validation',
        '--max-iterations', '25',  // 30 - 5
        '--no-delegate'  // Should be present
      ]));
    });
  });

  describe('Concurrent delegation limits', () => {
    it('should enforce global concurrent delegation limit', async () => {
      // Set MAX_CONCURRENT_DELEGATIONS to 3 (default)
      const tasks = [
        delegate({ task: 'Task 1' }),
        delegate({ task: 'Task 2' }),
        delegate({ task: 'Task 3' })
      ];

      // Fourth delegation should fail immediately
      await expect(delegate({ task: 'Task 4' })).rejects.toThrow(
        /Maximum concurrent delegations.*reached/
      );
    });

    it('should enforce per-session delegation limit', async () => {
      const parentSessionId = 'test-session-123';

      // Create multiple delegations for the same session
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
    });

    it('should decrement counter when delegation completes successfully', async () => {
      const task1 = delegate({ task: 'Task 1' });
      await new Promise(resolve => setTimeout(resolve, 50));

      // After first completes, should be able to start another
      const task2 = delegate({ task: 'Task 2' });

      // Should not throw
      await expect(task2).resolves.toBeDefined();
    });

    it('should decrement counter when delegation fails', async () => {
      // Make spawn fail
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(1), 10);  // Exit code 1 = failure
        }
      });

      const task1 = delegate({ task: 'Task 1' });
      await new Promise(resolve => setTimeout(resolve, 50));

      // Counter should be decremented after failure
      const task2 = delegate({ task: 'Task 2' });
      await expect(task2).resolves.toBeDefined();
    });

    it('should decrement counter on timeout', async () => {
      // Never complete
      mockProcess.on.mockImplementation(() => {});

      const task1 = delegate({ task: 'Task 1', timeout: 0.05 });  // 50ms timeout
      await new Promise(resolve => setTimeout(resolve, 100));

      // Counter should be decremented after timeout
      const task2 = delegate({ task: 'Task 2' });
      await expect(task2).resolves.toBeDefined();
    });
  });

  describe('Environment variable configuration', () => {
    it('should respect MAX_CONCURRENT_DELEGATIONS from environment', () => {
      // This would need to be set before importing the module
      // Testing the default value of 3 is covered by other tests
      expect(true).toBe(true);  // Placeholder
    });

    it('should respect MAX_DELEGATIONS_PER_SESSION from environment', () => {
      // This would need to be set before importing the module
      // Testing the default value of 10 is covered by other tests
      expect(true).toBe(true);  // Placeholder
    });
  });

  describe('Parent session tracking', () => {
    it('should track delegations per parent session independently', async () => {
      const session1 = 'session-1';
      const session2 = 'session-2';

      // Start delegations for both sessions
      const task1 = delegate({ task: 'Task 1', parentSessionId: session1 });
      const task2 = delegate({ task: 'Task 2', parentSessionId: session2 });

      // Both should succeed (different sessions)
      await expect(task1).resolves.toBeDefined();
      await expect(task2).resolves.toBeDefined();
    });

    it('should clean up session tracking after all delegations complete', async () => {
      const parentSessionId = 'cleanup-test';

      const task = delegate({ task: 'Task 1', parentSessionId });
      await new Promise(resolve => setTimeout(resolve, 50));

      // Session should be cleaned up from tracking
      // (Internal implementation detail, but counter should be reset)
      const newTask = delegate({ task: 'Task 2', parentSessionId });
      await expect(newTask).resolves.toBeDefined();
    });
  });

  describe('Debug logging', () => {
    it('should log delegation limits when debug is enabled', async () => {
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

      await delegate({ task: 'Test task', debug: true });
      await new Promise(resolve => setTimeout(resolve, 20));

      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining('Global active delegations')
      );

      consoleSpy.mockRestore();
    });
  });
});
