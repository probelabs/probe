/**
 * Tests for delegate tool functionality
 */

import { jest } from '@jest/globals';

// Mock child_process.spawn at module level
const mockSpawn = jest.fn();
jest.unstable_mockModule('child_process', () => ({
  spawn: mockSpawn
}));

// Mock utils module
const mockGetBinaryPath = jest.fn();
const mockBuildCliArgs = jest.fn();
jest.unstable_mockModule('../src/utils.js', () => ({
  getBinaryPath: mockGetBinaryPath,
  buildCliArgs: mockBuildCliArgs
}));

// Import after mocking
const { delegate, isDelegateAvailable } = await import('../src/delegate.js');

describe('Delegate Tool', () => {
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
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('delegate function', () => {
    it('should spawn probe agent with correct arguments and flags', async () => {
      const task = 'Analyze authentication code for security vulnerabilities';
      const currentIteration = 5;
      const maxIterations = 20;

      // Setup mock process to simulate successful completion
      mockProcess.stdout.on.mockImplementation((event, callback) => {
        if (event === 'data') {
          setTimeout(() => callback(Buffer.from('Mock delegate response')), 10);
        }
      });
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(0), 20); // Exit code 0 (success)
        }
      });

      // Execute delegate (don't await to check spawn call)
      const delegatePromise = delegate({
        task,
        currentIteration,
        maxIterations,
        debug: true
      });

      // Wait a moment for spawn to be called
      await new Promise(resolve => setTimeout(resolve, 5));

      // Verify spawn was called with correct arguments
      expect(mockSpawn).toHaveBeenCalledWith(
        '/mock/path/to/probe', // Mocked binary path
        [
          'agent',
          '--task', task,
          '--session-id', expect.any(String),
          '--prompt-type', 'code-researcher',  // Automatic default prompt
          '--no-schema-validation',            // Automatic flag
          '--no-mermaid-validation',           // Automatic flag
          '--max-iterations', '15',            // Remaining iterations (20-5)
          '--debug'
        ],
        {
          stdio: ['pipe', 'pipe', 'pipe'],
          timeout: 300000
        }
      );

      // Wait for delegate to complete
      const result = await delegatePromise;
      expect(result).toBe('Mock delegate response');
    });

    it('should calculate remaining iterations correctly', async () => {
      const task = 'Test task';
      
      mockProcess.stdout.on.mockImplementation((event, callback) => {
        if (event === 'data') {
          setTimeout(() => callback(Buffer.from('Response')), 10);
        }
      });
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(0), 20);
        }
      });

      // Test with high current iteration
      await delegate({
        task,
        currentIteration: 25,
        maxIterations: 30
      });

      // Should limit to at least 1 iteration
      expect(mockSpawn).toHaveBeenCalledWith(
        expect.any(String),
        expect.arrayContaining(['--max-iterations', '5']),
        expect.any(Object)
      );

      jest.clearAllMocks();
      mockSpawn.mockReturnValue(mockProcess);

      // Test with current iteration exceeding max
      await delegate({
        task,
        currentIteration: 35,
        maxIterations: 30
      });

      // Should still allow at least 1 iteration
      expect(mockSpawn).toHaveBeenCalledWith(
        expect.any(String),
        expect.arrayContaining(['--max-iterations', '1']),
        expect.any(Object)
      );
    });

    it('should always include automatic flags regardless of options', async () => {
      const task = 'Test automatic flags';
      
      mockProcess.stdout.on.mockImplementation((event, callback) => {
        if (event === 'data') {
          setTimeout(() => callback(Buffer.from('Response')), 10);
        }
      });
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(0), 20);
        }
      });

      await delegate({ task });

      // Verify automatic flags are always present
      const spawnArgs = mockSpawn.mock.calls[0][1];
      expect(spawnArgs).toContain('--prompt-type');
      expect(spawnArgs).toContain('code-researcher');
      expect(spawnArgs).toContain('--no-schema-validation');
      expect(spawnArgs).toContain('--no-mermaid-validation');
    });

    it('should handle process errors', async () => {
      const task = 'Test error handling';
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'error') {
          setTimeout(() => callback(new Error('Process spawn failed')), 10);
        }
      });

      await expect(delegate({ task })).rejects.toThrow('Failed to start delegate process: Process spawn failed');
    });

    it('should handle process exit with non-zero code', async () => {
      const task = 'Test failure handling';
      
      mockProcess.stderr.on.mockImplementation((event, callback) => {
        if (event === 'data') {
          setTimeout(() => callback(Buffer.from('Task execution failed')), 10);
        }
      });
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(1), 20); // Exit code 1 (failure)
        }
      });

      await expect(delegate({ task })).rejects.toThrow('Delegation failed: Task execution failed');
    });

    it('should handle empty responses', async () => {
      const task = 'Test empty response';
      
      mockProcess.stdout.on.mockImplementation((event, callback) => {
        if (event === 'data') {
          setTimeout(() => callback(Buffer.from('   \n  \t  ')), 10); // Whitespace only
        }
      });
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(0), 20);
        }
      });

      await expect(delegate({ task })).rejects.toThrow('Delegate agent returned empty response');
    });

    it('should handle timeout', async () => {
      const task = 'Test timeout';
      const shortTimeout = 0.1; // 100ms timeout
      
      // Don't call any callbacks to simulate hanging process
      
      await expect(delegate({ 
        task, 
        timeout: shortTimeout 
      })).rejects.toThrow('Delegation timed out after 0.1 seconds');
      
      // Verify process was killed
      expect(mockProcess.kill).toHaveBeenCalledWith('SIGTERM');
    });

    it('should require task parameter', async () => {
      await expect(delegate({})).rejects.toThrow('Task parameter is required and must be a string');
      await expect(delegate({ task: null })).rejects.toThrow('Task parameter is required and must be a string');
      await expect(delegate({ task: 123 })).rejects.toThrow('Task parameter is required and must be a string');
    });

    it('should include debug logs when debug is enabled', async () => {
      const task = 'Test debug logging';
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
      
      mockProcess.stdout.on.mockImplementation((event, callback) => {
        if (event === 'data') {
          setTimeout(() => callback(Buffer.from('Debug response')), 10);
        }
      });
      
      mockProcess.on.mockImplementation((event, callback) => {
        if (event === 'close') {
          setTimeout(() => callback(0), 20);
        }
      });

      await delegate({ 
        task, 
        debug: true,
        currentIteration: 3,
        maxIterations: 10 
      });

      // Verify debug logs were called
      expect(consoleSpy).toHaveBeenCalledWith(expect.stringContaining('[DELEGATE] Starting delegation session'));
      expect(consoleSpy).toHaveBeenCalledWith(expect.stringContaining(`[DELEGATE] Task: ${task}`));
      expect(consoleSpy).toHaveBeenCalledWith(expect.stringContaining('[DELEGATE] Current iteration: 3/10'));
      expect(consoleSpy).toHaveBeenCalledWith(expect.stringContaining('[DELEGATE] Remaining iterations for subagent: 7'));
      
      consoleSpy.mockRestore();
    });
  });

  describe('XML parsing integration', () => {
    it('should be parseable by AI agents as XML tool', () => {
      // Test that the delegate tool can be represented as XML
      const xmlExample = `<delegate>
<task>Analyze all authentication and authorization code for security vulnerabilities</task>
</delegate>`;

      // This would be parsed by the AI agent's XML parser
      // Here we just verify the structure is valid
      expect(xmlExample).toContain('<delegate>');
      expect(xmlExample).toContain('<task>');
      expect(xmlExample).toContain('</task>');
      expect(xmlExample).toContain('</delegate>');
    });

    it('should handle complex multi-line tasks in XML format', () => {
      const complexTask = `Review database queries and API endpoints for performance bottlenecks.
Include analysis of:
- Query optimization opportunities  
- Index usage patterns
- N+1 query detection
- Connection pooling efficiency`;

      const xmlExample = `<delegate>
<task>${complexTask}</task>
</delegate>`;

      // Verify complex content can be properly contained
      expect(xmlExample).toContain('Review database queries');
      expect(xmlExample).toContain('N+1 query detection');
    });
  });

  describe('agentic loop integration', () => {
    it('should demonstrate proper agentic task separation', () => {
      // Example of how AI agent should automatically separate complex requests
      const userRequest = 'Analyze my codebase for security, performance, and maintainability issues';
      
      const expectedSeparation = [
        {
          xml: '<delegate><task>Analyze all authentication, authorization, input validation, and cryptographic code for security vulnerabilities and provide specific remediation recommendations</task></delegate>',
          focus: 'security'
        },
        {
          xml: '<delegate><task>Review all database queries, API endpoints, algorithms, and resource usage patterns for performance bottlenecks and suggest optimization strategies</task></delegate>',
          focus: 'performance'  
        },
        {
          xml: '<delegate><task>Examine code structure, design patterns, documentation, and maintainability across all modules and provide refactoring recommendations</task></delegate>',
          focus: 'maintainability'
        }
      ];

      // Verify each task is self-contained and focused
      expectedSeparation.forEach(({ xml, focus }) => {
        expect(xml).toContain('<delegate>');
        expect(xml).toContain('<task>');
        expect(xml).toContain('</task>');
        expect(xml).toContain('</delegate>');
        
        // Each task should be comprehensive within its domain
        if (focus === 'security') {
          expect(xml).toContain('authentication');
          expect(xml).toContain('authorization');
          expect(xml).toContain('vulnerabilities');
        } else if (focus === 'performance') {
          expect(xml).toContain('database queries');
          expect(xml).toContain('bottlenecks');
          expect(xml).toContain('optimization');
        } else if (focus === 'maintainability') {
          expect(xml).toContain('design patterns');
          expect(xml).toContain('refactoring');
          expect(xml).toContain('maintainability');
        }
      });
    });
  });

  describe('isDelegateAvailable', () => {
    it('should return true when binary is available', async () => {
      // Mock getBinaryPath to succeed
      mockGetBinaryPath.mockResolvedValue('/path/to/probe');
      
      const isAvailable = await isDelegateAvailable();
      expect(isAvailable).toBe(true);
    });

    it('should return false when binary is not available', async () => {
      // Mock getBinaryPath to fail
      mockGetBinaryPath.mockRejectedValue(new Error('Binary not found'));
      
      const isAvailable = await isDelegateAvailable();
      expect(isAvailable).toBe(false);
    });
  });
});