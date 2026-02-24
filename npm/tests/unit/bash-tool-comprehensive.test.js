/**
 * Comprehensive integration test for the complete bash tool feature
 * This test validates the entire bash tool implementation from CLI to execution
 * @module tests/unit/bash-tool-comprehensive
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import os from 'os';
import path from 'path';

// Mock external dependencies
jest.mock('ai', () => ({
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

// Import the components we want to test
import { BashPermissionChecker } from '../../src/agent/bashPermissions.js';
import { executeBashCommand, formatExecutionResult } from '../../src/agent/bashExecutor.js';
import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from '../../src/agent/bashDefaults.js';
import { bashTool } from '../../src/tools/bash.js';

describe('Bash Tool - End-to-End Integration', () => {
  describe('Complete Permission Flow', () => {
    test('should implement secure-by-default approach', async () => {
      // Create a bash tool with default security settings
      const tool = bashTool({
        debug: false,
        bashConfig: { timeout: 5000 }
      });

      // Test that safe commands are allowed
      const safeCommands = [
        'ls -la',
        'pwd',
        'echo hello',
        'git status',
        'cat package.json',
        'grep "test" package.json',  // Changed from recursive to specific file
        'find ./src -name "*.js" -maxdepth 2',  // Limited depth to avoid timeout
        'npm list --depth=0'  // Limited depth for faster execution
      ];

      for (const command of safeCommands) {
        const result = await tool.execute({ command });
        expect(result).not.toContain('Permission denied');
      }

      // Test that dangerous commands are blocked
      const dangerousCommands = [
        'rm -rf /',
        'sudo apt-get install',
        'chmod 777 /',
        'npm install express',
        'git push origin main',
        'killall node',
        'shutdown now'
      ];

      for (const command of dangerousCommands) {
        const result = await tool.execute({ command });
        expect(result).toContain('Permission denied');
      }
    });

    test('should support custom configuration patterns', async () => {
      // Create tool with custom allow/deny patterns
      const tool = bashTool({
        debug: false,
        bashConfig: {
          allow: ['docker:ps', 'make:help'],
          deny: ['echo:dangerous'],
          timeout: 5000
        }
      });

      // Custom allow should work
      const result1 = await tool.execute({ command: 'docker ps' });
      expect(result1).not.toContain('Permission denied');

      // Custom deny should work
      const result2 = await tool.execute({ command: 'echo dangerous' });
      expect(result2).toContain('Permission denied');

      // Default allows should still work
      const result3 = await tool.execute({ command: 'ls' });
      expect(result3).not.toContain('Permission denied');
    });

    test('should support override modes', async () => {
      // Create tool with custom-only patterns
      const tool = bashTool({
        debug: false,
        bashConfig: {
          allow: ['echo', 'pwd'],
          deny: ['pwd:bad'],
          disableDefaultAllow: true,
          disableDefaultDeny: true,
          timeout: 5000
        }
      });

      // Only custom allows should work
      const result1 = await tool.execute({ command: 'echo hello' });
      expect(result1).not.toContain('Permission denied');

      // Default allows should be denied
      const result2 = await tool.execute({ command: 'ls' });
      expect(result2).toContain('not in allow list');

      // Custom deny should work
      const result3 = await tool.execute({ command: 'pwd bad' });
      expect(result3).toContain('Permission denied');

      // Default dangerous commands should be allowed (if in custom allow)
      const result4 = await tool.execute({ command: 'rm -rf /' });
      expect(result4).toContain('not in allow list'); // Not in custom allow
    });
  });

  describe('Real Command Execution Integration', () => {
    test('should execute real safe commands end-to-end', async () => {
      const tool = bashTool({
        debug: false,
        bashConfig: { timeout: 10000 }
      });

      // Test a simple, guaranteed-safe command
      const result = await tool.execute({ command: 'echo "integration test"' });
      
      expect(result).not.toContain('Permission denied');
      expect(result).not.toContain('Error:');
      expect(result).toContain('integration test');
    }, 15000);

    test('should handle command failures gracefully', async () => {
      const tool = bashTool({
        debug: false,
        bashConfig: { timeout: 5000 }
      });

      // Test a command that will fail but is allowed
      const result = await tool.execute({ command: 'ls /this/does/not/exist/surely' });
      
      expect(result).not.toContain('Permission denied');
      // Should indicate failure but not crash
      expect(result).toContain('Command failed') || expect(result).toContain('No such file');
    }, 10000);

    test('should respect timeout settings', async () => {
      const tool = bashTool({
        debug: false,
        bashConfig: { timeout: 1000 } // Very short timeout
      });

      // Test a command that would take longer than timeout
      const result = await tool.execute({ command: 'sleep 2' });
      
      expect(result).not.toContain('Permission denied');
      expect(result).toContain('timed out') || expect(result).toContain('Command failed');
    }, 5000);
  });

  describe('Pattern Matching Accuracy', () => {
    test('should match patterns precisely', () => {
      const checker = new BashPermissionChecker({ debug: false });

      // Test exact patterns  
      expect(checker.check('ls').allowed).toBe(true);
      expect(checker.check('ls -la').allowed).toBe(true); // ls pattern allows any ls args

      // Test wildcard patterns  
      expect(checker.check('git status').allowed).toBe(true);
      expect(checker.check('git log --oneline').allowed).toBe(true);
      expect(checker.check('git push origin main').allowed).toBe(false);

      // Test complex patterns
      expect(checker.check('npm list').allowed).toBe(true);
      expect(checker.check('npm install').allowed).toBe(false);
    });

    test('should have comprehensive default patterns', () => {
      // Verify we have substantial coverage
      expect(DEFAULT_ALLOW_PATTERNS.length).toBeGreaterThan(200);
      expect(DEFAULT_DENY_PATTERNS.length).toBeGreaterThan(100);

      // Spot check critical patterns
      expect(DEFAULT_ALLOW_PATTERNS).toContain('ls');
      expect(DEFAULT_ALLOW_PATTERNS).toContain('cat');
      expect(DEFAULT_ALLOW_PATTERNS).toContain('git:status');
      
      expect(DEFAULT_DENY_PATTERNS).toContain('rm:-rf');
      expect(DEFAULT_DENY_PATTERNS).toContain('sudo');
      expect(DEFAULT_DENY_PATTERNS).toContain('npm:install');
    });
  });

  describe('Error Handling and Validation', () => {
    test('should validate input parameters thoroughly', async () => {
      const tool = bashTool({ debug: false });

      // Test various invalid inputs
      const invalidInputs = [
        { command: '' },
        { command: null },
        { command: undefined },
        { command: '   ' },
        {},
        { command: 'echo hello', timeout: 'invalid' },
        { command: 'echo hello', timeout: -1 }
      ];

      for (const input of invalidInputs) {
        const result = await tool.execute(input);
        expect(result).toContain('Error');
        expect(typeof result).toBe('string');
      }
    });

    test('should handle working directory restrictions', async () => {
      const tempDir = os.tmpdir();
      // Create a path that is definitely outside tempDir
      const outsideDir = process.platform === 'win32' ? 'C:\\Windows' : '/usr';

      const tool = bashTool({
        debug: false,
        allowedFolders: [tempDir],
        bashConfig: { timeout: 5000 }
      });

      // Should work within allowed folder
      const result1 = await tool.execute({
        command: process.platform === 'win32' ? 'echo %CD%' : 'pwd',
        workingDirectory: tempDir
      });
      expect(result1).not.toContain('not within allowed folders');

      // Should fail outside allowed folders
      const result2 = await tool.execute({
        command: process.platform === 'win32' ? 'echo %CD%' : 'pwd',
        workingDirectory: outsideDir
      });
      expect(result2).toContain('not within allowed folders');
    });

    test('should provide helpful error messages', async () => {
      const tool = bashTool({ debug: false });

      const result = await tool.execute({ command: 'sudo rm -rf /' });
      
      expect(result).toContain('Permission denied');
      expect(result).toContain('Common reasons');
      expect(result).toContain('--bash-allow');
      expect(result).toContain('safe alternatives');
    });
  });

  describe('Configuration Flexibility', () => {
    test('should work with minimal configuration', () => {
      const tool = bashTool();
      
      expect(tool.name).toBe('bash');
      expect(typeof tool.execute).toBe('function');
      expect(tool.inputSchema).toBeDefined();
    });

    test('should work with maximal configuration', () => {
      const tool = bashTool({
        debug: true,
        cwd: '/project',
        allowedFolders: ['/project', '/tmp'],
        bashConfig: {
          allow: ['docker:*', 'make:*'],
          deny: ['rm:*', 'chmod:*'],
          disableDefaultAllow: false,
          disableDefaultDeny: false,
          timeout: 120000,
          workingDirectory: '/project',
          env: {
            NODE_ENV: 'test',
            DEBUG: '1'
          },
          maxBuffer: 50 * 1024 * 1024
        }
      });
      
      expect(tool.name).toBe('bash');
      expect(typeof tool.execute).toBe('function');
      expect(tool.inputSchema).toBeDefined();
    });
  });

  describe('Performance and Resource Management', () => {
    test('should handle multiple concurrent executions', async () => {
      const tool = bashTool({
        debug: false,
        bashConfig: { timeout: 5000 }
      });

      // Execute multiple commands concurrently
      const promises = Array.from({ length: 5 }, (_, i) =>
        tool.execute({ command: `echo "test ${i}"` })
      );

      const results = await Promise.all(promises);
      
      // All should succeed
      results.forEach((result, i) => {
        expect(result).not.toContain('Permission denied');
        expect(result).toContain(`test ${i}`);
      });
    }, 15000);

    test('should handle resource limits appropriately', () => {
      const result = formatExecutionResult({
        success: true,
        stdout: 'x'.repeat(1000),
        stderr: '',
        exitCode: 0,
        command: 'echo',
        duration: 123
      });

      expect(result).toContain('x'.repeat(1000));
      expect(typeof result).toBe('string');
    });
  });
});

describe('CLI Integration Simulation', () => {
  test('should simulate complete CLI workflow', () => {
    // Simulate CLI argument parsing
    const cliArgs = {
      enableBash: true,
      bashAllow: 'docker:ps,npm:run:*',
      bashDeny: 'rm:*,sudo:*',
      bashTimeout: '60000',
      defaultBashAllow: true,
      defaultBashDeny: true
    };

    // Process arguments like CLI would
    const bashConfig = {};
    
    if (cliArgs.bashAllow) {
      bashConfig.allow = cliArgs.bashAllow.split(',').map(p => p.trim()).filter(p => p.length > 0);
    }
    
    if (cliArgs.bashDeny) {
      bashConfig.deny = cliArgs.bashDeny.split(',').map(p => p.trim()).filter(p => p.length > 0);
    }
    
    if (cliArgs.bashTimeout) {
      const timeout = parseInt(cliArgs.bashTimeout, 10);
      if (!isNaN(timeout)) {
        bashConfig.timeout = timeout;
      }
    }
    
    bashConfig.disableDefaultAllow = cliArgs.defaultBashAllow === false;
    bashConfig.disableDefaultDeny = cliArgs.defaultBashDeny === false;

    // Create tool with processed config
    const tool = bashTool({
      debug: false,
      bashConfig
    });

    // Verify configuration was applied
    expect(tool.name).toBe('bash');
    expect(typeof tool.execute).toBe('function');
  });
});