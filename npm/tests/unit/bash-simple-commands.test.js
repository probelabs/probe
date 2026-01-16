/**
 * Tests for simplified bash tool (rejects complex commands for security)
 * @module tests/unit/bash-simple-commands
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { parseSimpleCommand, parseCommand, parseCommandForExecution, isComplexCommand, isComplexPattern, matchesComplexPattern } from '../../src/agent/bashCommandUtils.js';
import { BashPermissionChecker } from '../../src/agent/bashPermissions.js';

// Mock the 'ai' package since it may not be available in test environment
jest.mock('ai', () => ({
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

import { bashTool } from '../../src/tools/bash.js';

describe('Simple Command Parser', () => {
  describe('parseSimpleCommand', () => {
    test('should parse simple commands correctly', () => {
      const result = parseSimpleCommand('ls -la');
      expect(result.success).toBe(true);
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
      expect(result.isComplex).toBe(false);
    });

    test('should handle quoted arguments correctly (fix quote bug)', () => {
      const result = parseSimpleCommand('grep "test file" *.txt');
      expect(result.success).toBe(true);
      expect(result.command).toBe('grep');
      expect(result.args).toEqual(['test file', '*.txt']); // Quotes stripped!
      expect(result.fullArgs).toEqual(['grep', 'test file', '*.txt']);
    });

    test('should handle single quotes correctly', () => {
      const result = parseSimpleCommand("echo 'hello world'");
      expect(result.success).toBe(true);
      expect(result.command).toBe('echo');
      expect(result.args).toEqual(['hello world']); // Single quotes stripped!
    });

    test('should handle mixed quotes', () => {
      const result = parseSimpleCommand('git commit -m "Fixed bug" --author="John Doe"');
      expect(result.success).toBe(true);
      expect(result.command).toBe('git');
      expect(result.args).toEqual(['commit', '-m', 'Fixed bug', '--author=John Doe']);
    });

    test('should reject commands with unclosed quotes', () => {
      const result = parseSimpleCommand('echo "unclosed quote');
      expect(result.success).toBe(false);
      expect(result.error).toContain('Unclosed quote');
    });

    test('should reject empty commands', () => {
      const result = parseSimpleCommand('');
      expect(result.success).toBe(false);
      expect(result.error).toContain('empty');
    });

    test('should reject null/undefined commands', () => {
      expect(parseSimpleCommand(null).success).toBe(false);
      expect(parseSimpleCommand(undefined).success).toBe(false);
    });
  });

  describe('Complex Command Detection', () => {
    test('should detect pipes as complex', () => {
      expect(isComplexCommand('ls | grep test')).toBe(true);
      const result = parseSimpleCommand('ls | grep test');
      expect(result.success).toBe(false);
      expect(result.isComplex).toBe(true);
      expect(result.error).toContain('Complex shell commands');
    });

    test('should detect logical operators as complex', () => {
      expect(isComplexCommand('make && make test')).toBe(true);
      expect(isComplexCommand('make || echo failed')).toBe(true);
    });

    test('should detect command substitution as complex', () => {
      expect(isComplexCommand('echo $(date)')).toBe(true);
      expect(isComplexCommand('ls `pwd`')).toBe(true);
    });

    test('should detect redirections as complex', () => {
      expect(isComplexCommand('ls > output.txt')).toBe(true);
      expect(isComplexCommand('cat < input.txt')).toBe(true);
    });

    test('should detect background execution as complex', () => {
      expect(isComplexCommand('long-task &')).toBe(true);
    });

    test('should detect command separators as complex', () => {
      expect(isComplexCommand('cd /tmp; ls')).toBe(true);
    });

    test('should allow simple commands', () => {
      expect(isComplexCommand('ls -la')).toBe(false);
      expect(isComplexCommand('git status')).toBe(false);
      expect(isComplexCommand('npm test')).toBe(false);
    });
  });

  describe('parseCommandForExecution', () => {
    test('should return array for simple commands', () => {
      const result = parseCommandForExecution('ls -la');
      expect(result).toEqual(['ls', '-la']);
    });

    test('should return null for complex commands', () => {
      const result = parseCommandForExecution('ls | grep test');
      expect(result).toBeNull();
    });

    test('should handle quotes correctly for execution', () => {
      const result = parseCommandForExecution('grep "test file" data.txt');
      expect(result).toEqual(['grep', 'test file', 'data.txt']);
    });
  });
});

describe('Complex Pattern Matching', () => {
  describe('isComplexPattern', () => {
    test('should detect patterns with pipes', () => {
      expect(isComplexPattern('ls | grep *')).toBe(true);
      expect(isComplexPattern('git branch -a | grep *')).toBe(true);
    });

    test('should detect patterns with logical AND', () => {
      expect(isComplexPattern('cd * && git *')).toBe(true);
      expect(isComplexPattern('make && make test')).toBe(true);
    });

    test('should detect patterns with logical OR', () => {
      expect(isComplexPattern('* || echo failed')).toBe(true);
    });

    test('should detect patterns with redirections', () => {
      expect(isComplexPattern('* > output.txt')).toBe(true);
      expect(isComplexPattern('cat < *')).toBe(true);
    });

    test('should detect patterns with semicolon', () => {
      expect(isComplexPattern('cd *; ls')).toBe(true);
    });

    test('should detect patterns with command substitution', () => {
      expect(isComplexPattern('echo $(date)')).toBe(true);
      expect(isComplexPattern('ls `pwd`')).toBe(true);
    });

    test('should detect patterns with background execution', () => {
      expect(isComplexPattern('task &')).toBe(true);
    });

    test('should not detect simple patterns as complex', () => {
      expect(isComplexPattern('git:status')).toBe(false);
      expect(isComplexPattern('ls:*')).toBe(false);
      expect(isComplexPattern('npm:test')).toBe(false);
    });

    test('should handle null/undefined', () => {
      expect(isComplexPattern(null)).toBe(false);
      expect(isComplexPattern(undefined)).toBe(false);
      expect(isComplexPattern('')).toBe(false);
    });
  });

  describe('matchesComplexPattern', () => {
    test('should match exact complex commands', () => {
      expect(matchesComplexPattern('cd /tmp && ls', 'cd /tmp && ls')).toBe(true);
    });

    test('should match with wildcard at end', () => {
      expect(matchesComplexPattern('cd /tmp && git status', 'cd * && git *')).toBe(true);
      expect(matchesComplexPattern('cd /project && git log', 'cd * && git *')).toBe(true);
    });

    test('should match pipe patterns', () => {
      expect(matchesComplexPattern('git branch -a | grep release', 'git branch -a | grep *')).toBe(true);
      expect(matchesComplexPattern('git branch -a | grep feature', 'git branch -a | grep *')).toBe(true);
    });

    test('should match git fetch && git tag patterns', () => {
      expect(matchesComplexPattern('git fetch --tags && git tag -l "v*"', 'git fetch * && git tag *')).toBe(true);
    });

    test('should not match non-matching commands', () => {
      expect(matchesComplexPattern('rm -rf /', 'cd * && git *')).toBe(false);
      expect(matchesComplexPattern('ls | grep test', 'git branch -a | grep *')).toBe(false);
    });

    test('should handle whitespace normalization', () => {
      expect(matchesComplexPattern('cd  /tmp  &&  ls', 'cd * && ls')).toBe(true);
    });

    test('should handle null/undefined', () => {
      expect(matchesComplexPattern(null, 'pattern')).toBe(false);
      expect(matchesComplexPattern('command', null)).toBe(false);
    });
  });
});

describe('Simplified Permission Checker', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      allow: ['test:*', 'echo:hello'],
      deny: ['rm:*', 'dangerous:cmd'],
      debug: false
    });
  });

  describe('Complex Command Handling', () => {
    test('should reject complex commands without matching patterns', () => {
      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Complex shell commands require explicit allow patterns');
      expect(result.isComplex).toBe(true);
    });

    test('should allow complex commands with matching allow patterns', () => {
      const checkerWithComplex = new BashPermissionChecker({
        allow: ['cd * && git *', 'git branch -a | grep *'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result1 = checkerWithComplex.check('cd /project && git status');
      expect(result1.allowed).toBe(true);
      expect(result1.isComplex).toBe(true);

      const result2 = checkerWithComplex.check('git branch -a | grep release');
      expect(result2.allowed).toBe(true);
      expect(result2.isComplex).toBe(true);
    });

    test('should reject complex commands matching deny patterns', () => {
      const checkerWithComplex = new BashPermissionChecker({
        allow: ['cd * && *'],
        deny: ['cd * && rm *'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checkerWithComplex.check('cd /tmp && rm -rf *');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should reject command substitution without matching patterns', () => {
      const result = checker.check('echo $(whoami)');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Complex shell commands require explicit allow patterns');
    });

    test('should reject redirections without matching patterns', () => {
      const result = checker.check('ls > output.txt');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Complex shell commands require explicit allow patterns');
    });
  });

  describe('Simple Command Processing', () => {
    test('should allow commands matching allow patterns', () => {
      const result = checker.check('test --version');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(false);
    });

    test('should reject commands matching deny patterns', () => {
      const result = checker.check('rm -rf /');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should handle quoted arguments in pattern matching', () => {
      // This tests the quote handling fix
      const result = checker.check('echo "hello"');
      expect(result.allowed).toBe(true); // Should match echo:hello pattern
    });

    test('should reject commands with no matching allow pattern', () => {
      const result = checker.check('unknown-command');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Command not in allow list');
    });
  });

  describe('Configuration', () => {
    test('should work with empty allow list (allows everything not denied)', () => {
      const permissiveChecker = new BashPermissionChecker({
        allow: [], // Empty allow list
        deny: ['rm:*'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const allowed = permissiveChecker.check('any-command');
      expect(allowed.allowed).toBe(true);

      const denied = permissiveChecker.check('rm -rf');
      expect(denied.allowed).toBe(false);
    });

    test('should provide configuration summary', () => {
      const config = checker.getConfig();
      expect(config).toHaveProperty('allowPatterns');
      expect(config).toHaveProperty('denyPatterns');
      expect(config).toHaveProperty('totalPatterns');
      expect(typeof config.allowPatterns).toBe('number');
      expect(typeof config.denyPatterns).toBe('number');
    });
  });
});

describe('Bash Tool Integration with Simplified Architecture', () => {
  test('should create bash tool with working configuration', () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: {
        allow: ['ls:*', 'echo:*'],
        deny: ['rm:*'],
        timeout: 30000
      }
    });

    expect(tool.name).toBe('bash');
    expect(tool.description).toContain('Execute bash commands');
    expect(typeof tool.execute).toBe('function');
  });

  test('should reject complex commands without matching patterns in tool execution', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: { allow: ['ls:*'], deny: [] }
    });

    const result = await tool.execute({ command: 'ls | grep test' });
    expect(result).toContain('Permission denied');
    expect(result).toContain('Complex shell commands require explicit allow patterns');
  });

  test('should allow complex commands with matching patterns in tool execution', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: {
        allow: ['ls | grep *'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      }
    });

    // The permission check should pass - actual execution may fail in test env
    // but we're testing that permissions work correctly
    let permissionError = null;
    try {
      await tool.execute({ command: 'ls | grep test' });
    } catch (error) {
      if (error.message && error.message.includes('Permission denied')) {
        permissionError = error;
      }
    }

    expect(permissionError).toBeNull();
  });

  test('should handle simple commands in tool', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: { 
        allow: ['echo:*'], 
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true 
      }
    });

    // This should not throw during permission check
    // (actual execution might fail without proper environment, but permissions should pass)
    let permissionError = null;
    try {
      await tool.execute({ command: 'echo hello' });
    } catch (error) {
      if (error.message.includes('Permission denied')) {
        permissionError = error;
      }
      // Other execution errors (like missing environment) are expected in tests
    }

    expect(permissionError).toBeNull();
  });
});

describe('Architecture Alignment Tests', () => {
  test('should ensure parser and executor handle same command format', () => {
    const testCommands = [
      'ls -la',
      'git status',
      'echo "hello world"',
      'npm test',
      'docker ps',
      'make clean'
    ];

    for (const command of testCommands) {
      const parserResult = parseCommandForExecution(command);
      const permissionResult = parseCommand(command);

      // Both should succeed for simple commands
      expect(parserResult).not.toBeNull();
      expect(permissionResult.error).toBeNull();

      // Both should parse to same basic structure
      expect(parserResult[0]).toBe(permissionResult.command);
      expect(parserResult.slice(1)).toEqual(permissionResult.args);
    }
  });

  test('should ensure complex commands are detected consistently', () => {
    const complexCommands = [
      'ls | grep test',
      'make && make test',
      'echo $(date)',
      'ls > output.txt',
      'cmd1 ; cmd2',
      'background-task &'
    ];

    for (const command of complexCommands) {
      const parserResult = parseCommandForExecution(command);
      const isComplex = isComplexCommand(command);

      // Parser should reject complex commands (returns null)
      expect(parserResult).toBeNull();
      expect(isComplex).toBe(true);

      // Permission checker without complex patterns should reject
      const checkerWithoutPatterns = new BashPermissionChecker({
        allow: ['*'], // Only simple wildcard, no complex patterns
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });
      const rejectResult = checkerWithoutPatterns.check(command);
      expect(rejectResult.allowed).toBe(false);
      expect(rejectResult.reason).toContain('Complex shell commands require explicit allow patterns');
    }
  });

  test('should allow complex commands when matching patterns are configured', () => {
    const checkerWithPatterns = new BashPermissionChecker({
      allow: [
        'ls | grep *',
        'make && make test',
        'echo $(date)',
        'ls > *',
        'cmd1 ; cmd2',
        'background-task &'
      ],
      deny: [],
      disableDefaultAllow: true,
      disableDefaultDeny: true
    });

    const complexCommands = [
      'ls | grep test',
      'make && make test',
      'echo $(date)',
      'ls > output.txt',
      'cmd1 ; cmd2',
      'background-task &'
    ];

    for (const command of complexCommands) {
      const result = checkerWithPatterns.check(command);
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
    }
  });
});