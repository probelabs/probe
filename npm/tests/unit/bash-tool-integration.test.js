/**
 * Integration tests for bash tool with Vercel AI SDK
 * @module tests/unit/bash-tool-integration
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';

// Mock the 'ai' package since it may not be available in test environment
jest.mock('ai', () => ({
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

// Import after mocking
import { bashTool } from '../../src/tools/bash.js';

describe('Bash Tool Integration', () => {
  let tool;

  beforeEach(() => {
    tool = bashTool({
      debug: false,
      bashConfig: {
        timeout: 5000
      }
    });
  });

  describe('Tool Configuration', () => {
    test('should have correct name and description', () => {
      expect(tool.name).toBe('bash');
      expect(tool.description).toContain('Execute bash commands');
      expect(tool.description).toContain('security');
    });

    test('should have correct input schema', () => {
      expect(tool.inputSchema.type).toBe('object');
      expect(tool.inputSchema.properties.command).toBeDefined();
      expect(tool.inputSchema.properties.command.type).toBe('string');
      expect(tool.inputSchema.required).toContain('command');
      
      // Optional parameters
      expect(tool.inputSchema.properties.workingDirectory).toBeDefined();
      expect(tool.inputSchema.properties.timeout).toBeDefined();
      expect(tool.inputSchema.properties.env).toBeDefined();
    });

    test('should have execute function', () => {
      expect(typeof tool.execute).toBe('function');
    });
  });

  describe('Command Validation', () => {
    test('should reject empty commands', async () => {
      const result = await tool.execute({ command: '' });
      expect(result).toContain('Error: Command cannot be empty');
    });

    test('should reject null commands', async () => {
      const result = await tool.execute({ command: null });
      expect(result).toContain('Error: Command is required');
    });

    test('should reject undefined commands', async () => {
      const result = await tool.execute({});
      expect(result).toContain('Error: Command is required');
    });

    test('should reject whitespace-only commands', async () => {
      const result = await tool.execute({ command: '   ' });
      expect(result).toContain('Error: Command cannot be empty');
    });
  });

  describe('Permission Enforcement', () => {
    test('should deny dangerous commands', async () => {
      const dangerousCommands = [
        'rm -rf /',
        'sudo apt-get install something',
        'chmod 777 /',
        'npm install express',
        'git push origin main',
        'killall node'
      ];

      for (const command of dangerousCommands) {
        const result = await tool.execute({ command });
        expect(result).toContain('Permission denied');
        expect(result).not.toContain('Command completed');
      }
    });

    test('should provide helpful error messages for denied commands', async () => {
      const result = await tool.execute({ command: 'rm -rf /' });
      
      expect(result).toContain('Permission denied');
      expect(result).toContain('potentially dangerous');
      expect(result).toContain('Common reasons');
      expect(result).toContain('--bash-allow');
    });
  });

  describe('Safe Command Execution', () => {
    test('should execute safe commands', async () => {
      const result = await tool.execute({ command: 'echo "test"' });
      
      // Should not be a permission error
      expect(result).not.toContain('Permission denied');
      
      // Should contain the output or indicate successful execution
      expect(result).toContain('test');
    }, 10000);

    test('should handle command execution errors gracefully', async () => {
      const result = await tool.execute({ command: 'ls /nonexistent/path/12345' });
      
      // Should not be a permission error
      expect(result).not.toContain('Permission denied');
      
      // Should indicate execution failure
      expect(result).toMatch(/Command failed|Error executing/);
    });
  });

  describe('Custom Configuration', () => {
    test('should use custom allow patterns', async () => {
      const customTool = bashTool({
        debug: false,
        bashConfig: {
          allow: ['docker:ps'],
          timeout: 5000
        }
      });

      const result = await customTool.execute({ command: 'docker ps' });
      
      // Should not be denied by permissions (might fail due to docker not being available)
      expect(result).not.toContain('Permission denied');
    });

    test('should use custom deny patterns', async () => {
      const customTool = bashTool({
        debug: false,
        bashConfig: {
          deny: ['echo'],
          timeout: 5000
        }
      });

      const result = await customTool.execute({ command: 'echo hello' });
      expect(result).toContain('Permission denied');
    });

    test('should disable default patterns when requested', async () => {
      const customTool = bashTool({
        debug: false,
        bashConfig: {
          allow: ['echo'],
          disableDefaultAllow: true,
          disableDefaultDeny: true,
          timeout: 5000
        }
      });

      // Echo should work
      let result = await customTool.execute({ command: 'echo hello' });
      expect(result).not.toContain('Permission denied');

      // ls should be denied (not in custom allow list)
      result = await customTool.execute({ command: 'ls' });
      expect(result).toContain('not in allow list');
    });
  });

  describe('Working Directory and Options', () => {
    test('should handle working directory parameter', async () => {
      const result = await tool.execute({ 
        command: 'pwd',
        workingDirectory: '/tmp'
      });
      
      expect(result).not.toContain('Permission denied');
      
      // Should either succeed or fail gracefully
      if (!result.includes('Error:')) {
        expect(result).toContain('/tmp');
      }
    }, 10000);

    test('should handle timeout parameter', async () => {
      const result = await tool.execute({ 
        command: 'echo hello',
        timeout: 1000
      });
      
      expect(result).not.toContain('Permission denied');
    });

    test('should handle env parameter', async () => {
      const result = await tool.execute({ 
        command: 'printenv TEST_VAR',
        env: { TEST_VAR: 'test_value' }
      });
      
      expect(result).not.toContain('Permission denied');
      
      if (!result.includes('Error:')) {
        expect(result).toContain('test_value');
      }
    }, 10000);

    test('should reject invalid timeout values', async () => {
      const result = await tool.execute({ 
        command: 'echo hello',
        timeout: 'invalid'
      });
      
      expect(result).toContain('Error: Invalid execution options');
    });
  });

  describe('Folder Restrictions', () => {
    test('should respect allowedFolders configuration', async () => {
      const restrictedTool = bashTool({
        debug: false,
        allowedFolders: ['/tmp'],
        bashConfig: { timeout: 5000 }
      });

      // Should allow execution in allowed folder
      let result = await restrictedTool.execute({ 
        command: 'pwd',
        workingDirectory: '/tmp'
      });
      expect(result).not.toContain('not within allowed folders');

      // Should deny execution outside allowed folders
      result = await restrictedTool.execute({ 
        command: 'pwd',
        workingDirectory: '/usr'
      });
      expect(result).toContain('not within allowed folders');
    });
  });

  describe('Error Handling', () => {
    test('should handle tool execution errors gracefully', async () => {
      // Mock an execution error by providing invalid parameters
      const result = await tool.execute({ 
        command: 'echo hello',
        timeout: -1 // Invalid timeout
      });
      
      expect(typeof result).toBe('string');
      expect(result).toContain('Error');
    });

    test('should handle permission checker errors', async () => {
      // This should still work as permission checker is robust
      const result = await tool.execute({ command: 'ls' });
      expect(typeof result).toBe('string');
    });
  });
});

describe('Bash Tool with Different Configurations', () => {
  test('should work with minimal configuration', () => {
    const minimalTool = bashTool();
    
    expect(minimalTool.name).toBe('bash');
    expect(typeof minimalTool.execute).toBe('function');
  });

  test('should work with full configuration', () => {
    const fullTool = bashTool({
      debug: true,
      cwd: '/home/test',
      allowedFolders: ['/home/test', '/tmp'],
      bashConfig: {
        allow: ['docker:*'],
        deny: ['rm:*'],
        disableDefaultAllow: false,
        disableDefaultDeny: false,
        timeout: 60000,
        workingDirectory: '/home/test',
        env: { NODE_ENV: 'test' },
        maxBuffer: 5 * 1024 * 1024
      }
    });
    
    expect(fullTool.name).toBe('bash');
    expect(typeof fullTool.execute).toBe('function');
  });

  test('should handle debug mode', () => {
    const debugTool = bashTool({
      debug: true,
      bashConfig: { timeout: 5000 }
    });
    
    expect(debugTool.name).toBe('bash');
    
    // Debug mode should not affect basic functionality
    expect(typeof debugTool.execute).toBe('function');
  });
});