/**
 * Tests for bash tool functionality
 * @module tests/unit/bash-tool
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { BashPermissionChecker, parseCommand, matchesPattern } from '../../src/agent/bashPermissions.js';
import { executeBashCommand, formatExecutionResult, validateExecutionOptions } from '../../src/agent/bashExecutor.js';
import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from '../../src/agent/bashDefaults.js';

describe('Bash Permission Checker', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      debug: false
    });
  });

  describe('parseCommand', () => {
    test('should parse simple commands', () => {
      const result = parseCommand('ls -la');
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
      expect(result.full).toBe('ls -la');
    });

    test('should parse commands with multiple arguments', () => {
      const result = parseCommand('git log --oneline -10');
      expect(result.command).toBe('git');
      expect(result.args).toEqual(['log', '--oneline', '-10']);
    });

    test('should handle quoted arguments', () => {
      const result = parseCommand('grep -r "TODO" src/');
      expect(result.command).toBe('grep');
      expect(result.args).toEqual(['-r', '"TODO"', 'src/']);
    });

    test('should handle empty commands', () => {
      const result = parseCommand('');
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
    });

    test('should handle null/undefined commands', () => {
      const result = parseCommand(null);
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
    });

    test('should handle commands with extra whitespace', () => {
      const result = parseCommand('  ls   -la   ');
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
    });
  });

  describe('matchesPattern', () => {
    test('should match exact commands with no args', () => {
      const parsed = { command: 'ls', args: [] };
      expect(matchesPattern(parsed, 'ls')).toBe(true);
      expect(matchesPattern(parsed, 'cat')).toBe(false);
    });

    test('should not match exact pattern when args are present', () => {
      const parsed = { command: 'ls', args: ['-la'] };
      expect(matchesPattern(parsed, 'ls')).toBe(false); // Exact match requires no args
    });

    test('should match wildcard patterns', () => {
      const parsed = { command: 'ls', args: ['-la', 'src/'] };
      expect(matchesPattern(parsed, 'ls:*')).toBe(true);
      expect(matchesPattern(parsed, 'cat:*')).toBe(false);
    });

    test('should match specific argument patterns', () => {
      const parsed = { command: 'git', args: ['status'] };
      expect(matchesPattern(parsed, 'git:status')).toBe(true);
      expect(matchesPattern(parsed, 'git:log')).toBe(false);
    });

    test('should match partial wildcard patterns', () => {
      const parsed = { command: 'git', args: ['log', '--oneline', '-10'] };
      expect(matchesPattern(parsed, 'git:log:*')).toBe(true);
      expect(matchesPattern(parsed, 'git:status:*')).toBe(false);
    });

    test('should match complex patterns with multiple specific args', () => {
      const parsed = { command: 'npm', args: ['run', 'test'] };
      expect(matchesPattern(parsed, 'npm:run:test')).toBe(true);
      expect(matchesPattern(parsed, 'npm:run:build')).toBe(false);
      expect(matchesPattern(parsed, 'npm:install:*')).toBe(false);
    });

    test('should handle empty or invalid patterns', () => {
      const parsed = { command: 'ls', args: [] };
      expect(matchesPattern(parsed, '')).toBe(false);
      expect(matchesPattern(parsed, null)).toBe(false);
      expect(matchesPattern(parsed, undefined)).toBe(false);
    });
  });

  describe('Default Patterns', () => {
    test('should have comprehensive allow patterns', () => {
      expect(DEFAULT_ALLOW_PATTERNS).toBeDefined();
      expect(Array.isArray(DEFAULT_ALLOW_PATTERNS)).toBe(true);
      expect(DEFAULT_ALLOW_PATTERNS.length).toBeGreaterThan(50);
      
      // Check for key patterns
      expect(DEFAULT_ALLOW_PATTERNS).toContain('ls');
      expect(DEFAULT_ALLOW_PATTERNS).toContain('ls:*');
      expect(DEFAULT_ALLOW_PATTERNS).toContain('git:status');
      expect(DEFAULT_ALLOW_PATTERNS).toContain('cat:*');
    });

    test('should have comprehensive deny patterns', () => {
      expect(DEFAULT_DENY_PATTERNS).toBeDefined();
      expect(Array.isArray(DEFAULT_DENY_PATTERNS)).toBe(true);
      expect(DEFAULT_DENY_PATTERNS.length).toBeGreaterThan(20);
      
      // Check for key dangerous patterns
      expect(DEFAULT_DENY_PATTERNS).toContain('rm:-rf');
      expect(DEFAULT_DENY_PATTERNS).toContain('sudo:*');
      expect(DEFAULT_DENY_PATTERNS).toContain('npm:install');
    });
  });

  describe('Permission Checking with Defaults', () => {
    test('should allow safe commands by default', () => {
      const commands = [
        'ls -la',
        'pwd',
        'cat package.json',
        'git status',
        'git log --oneline',
        'npm list',
        'echo hello',
        'find . -name "*.js"',
        'grep -r "TODO" src/'
      ];

      commands.forEach(command => {
        const result = checker.check(command);
        expect(result.allowed).toBe(true);
      });
    });

    test('should deny dangerous commands by default', () => {
      const commands = [
        'rm -rf /',
        'rm -rf src/',
        'sudo apt-get install',
        'sudo rm -rf',
        'chmod 777 /',
        'npm install express',
        'pip install django',
        'git push origin main',
        'killall node',
        'shutdown now'
      ];

      commands.forEach(command => {
        const result = checker.check(command);
        expect(result.allowed).toBe(false);
        expect(result.reason).toContain('matches deny pattern');
      });
    });

    test('should handle edge cases', () => {
      // Empty command
      expect(checker.check('').allowed).toBe(false);
      
      // Null command
      expect(checker.check(null).allowed).toBe(false);
      
      // Undefined command
      expect(checker.check(undefined).allowed).toBe(false);
      
      // Command not in lists
      expect(checker.check('unknowncommand').allowed).toBe(false);
    });
  });

  describe('Custom Configuration', () => {
    test('should handle custom allow patterns', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['docker:ps', 'docker:images'],
        debug: false
      });

      expect(customChecker.check('docker ps').allowed).toBe(true);
      expect(customChecker.check('docker images').allowed).toBe(true);
      expect(customChecker.check('docker run nginx').allowed).toBe(false); // Not explicitly allowed
    });

    test('should handle custom deny patterns', () => {
      const customChecker = new BashPermissionChecker({
        deny: ['git:push', 'npm:publish'],
        debug: false
      });

      expect(customChecker.check('git push origin main').allowed).toBe(false);
      expect(customChecker.check('npm publish').allowed).toBe(false);
      expect(customChecker.check('git status').allowed).toBe(true); // Still in default allow
    });

    test('should disable default allow list when requested', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['echo'],
        disableDefaultAllow: true,
        debug: false
      });

      // Only echo should work
      expect(customChecker.check('echo hello').allowed).toBe(true);
      
      // Default allow commands should be denied
      expect(customChecker.check('ls -la').allowed).toBe(false);
      expect(customChecker.check('git status').allowed).toBe(false);
    });

    test('should disable default deny list when requested', () => {
      const customChecker = new BashPermissionChecker({
        deny: ['echo'],
        disableDefaultDeny: true,
        debug: false
      });

      // Custom deny should work
      expect(customChecker.check('echo hello').allowed).toBe(false);
      
      // Default deny commands should be allowed (if in allow list)
      expect(customChecker.check('sudo something').allowed).toBe(false); // Still not in allow list
    });

    test('should handle completely custom lists', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['echo', 'pwd'],
        deny: ['echo:bad'],
        disableDefaultAllow: true,
        disableDefaultDeny: true,
        debug: false
      });

      expect(customChecker.check('echo hello').allowed).toBe(true);
      expect(customChecker.check('echo bad').allowed).toBe(false); // Custom deny
      expect(customChecker.check('pwd').allowed).toBe(true);
      expect(customChecker.check('ls').allowed).toBe(false); // Not in custom allow
      expect(customChecker.check('rm -rf /').allowed).toBe(false); // Not in custom allow
    });
  });

  describe('getConfig', () => {
    test('should return configuration info', () => {
      const config = checker.getConfig();
      expect(config).toHaveProperty('allowPatterns');
      expect(config).toHaveProperty('denyPatterns');
      expect(config).toHaveProperty('totalPatterns');
      expect(typeof config.allowPatterns).toBe('number');
      expect(typeof config.denyPatterns).toBe('number');
      expect(config.totalPatterns).toBe(config.allowPatterns + config.denyPatterns);
    });
  });
});

describe('Bash Command Executor', () => {
  test('should execute simple safe commands', async () => {
    const result = await executeBashCommand('echo "hello world"', {
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(true);
    expect(result.stdout.trim()).toBe('hello world');
    expect(result.exitCode).toBe(0);
    expect(result.command).toBe('echo "hello world"');
    expect(typeof result.duration).toBe('number');
  }, 10000);

  test('should handle command timeouts', async () => {
    const result = await executeBashCommand('sleep 2', {
      timeout: 500, // Very short timeout
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.killed).toBe(true);
    expect(result.error).toContain('timed out');
  }, 10000);

  test('should handle non-existent commands', async () => {
    const result = await executeBashCommand('nonexistentcommand123456', {
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain('Failed to execute command');
  });

  test('should handle commands with non-zero exit codes', async () => {
    // Use a command that will definitely fail
    const result = await executeBashCommand('ls /this/directory/does/not/exist/surely', {
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.length).toBeGreaterThan(0);
  });

  test('should respect working directory', async () => {
    const result = await executeBashCommand('pwd', {
      workingDirectory: '/tmp',
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(true);
    expect(result.stdout.trim()).toBe('/tmp');
  });

  test('should handle invalid working directory', async () => {
    const result = await executeBashCommand('pwd', {
      workingDirectory: '/this/path/does/not/exist/surely',
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain('Invalid working directory');
  });

  test('should handle environment variables', async () => {
    const result = await executeBashCommand('echo $TEST_VAR', {
      env: { TEST_VAR: 'test_value' },
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(true);
    expect(result.stdout.trim()).toBe('test_value');
  });
});

describe('validateExecutionOptions', () => {
  test('should accept valid options', () => {
    const validation = validateExecutionOptions({
      timeout: 60000,
      maxBuffer: 1024 * 1024,
      workingDirectory: '/tmp',
      env: { NODE_ENV: 'test' }
    });

    expect(validation.valid).toBe(true);
    expect(validation.errors).toHaveLength(0);
  });

  test('should reject invalid timeout', () => {
    const validation = validateExecutionOptions({
      timeout: 'invalid'
    });

    expect(validation.valid).toBe(false);
    expect(validation.errors).toContain('timeout must be a non-negative number');
  });

  test('should reject very small maxBuffer', () => {
    const validation = validateExecutionOptions({
      maxBuffer: 100
    });

    expect(validation.valid).toBe(false);
    expect(validation.errors).toContain('maxBuffer must be at least 1024 bytes');
  });

  test('should warn about high values', () => {
    const validation = validateExecutionOptions({
      timeout: 700000, // > 10 minutes
      maxBuffer: 200 * 1024 * 1024 // > 100MB
    });

    expect(validation.valid).toBe(true);
    expect(validation.warnings).toContain('timeout is very high (>10 minutes)');
    expect(validation.warnings).toContain('maxBuffer is very high (>100MB)');
  });
});

describe('formatExecutionResult', () => {
  test('should format successful results', () => {
    const result = {
      success: true,
      stdout: 'hello world',
      stderr: '',
      exitCode: 0,
      command: 'echo hello world',
      duration: 100,
      workingDirectory: '/test'
    };

    const formatted = formatExecutionResult(result);
    expect(formatted).toBe('hello world');
  });

  test('should format results with stderr', () => {
    const result = {
      success: false,
      stdout: '',
      stderr: 'command not found',
      exitCode: 127,
      command: 'badcommand',
      duration: 50,
      workingDirectory: '/test'
    };

    const formatted = formatExecutionResult(result);
    expect(formatted).toContain('command not found');
  });

  test('should include metadata when requested', () => {
    const result = {
      success: true,
      stdout: 'output',
      stderr: '',
      exitCode: 0,
      command: 'test command',
      workingDirectory: '/test',
      duration: 123
    };

    const formatted = formatExecutionResult(result, true);
    expect(formatted).toContain('Command: test command');
    expect(formatted).toContain('Duration: 123ms');
    expect(formatted).toContain('Working Directory: /test');
    expect(formatted).toContain('Exit Code: 0');
  });

  test('should handle empty results', () => {
    const result = {
      success: true,
      stdout: '',
      stderr: '',
      exitCode: 0,
      command: 'true',
      duration: 10,
      workingDirectory: '/test'
    };

    const formatted = formatExecutionResult(result);
    expect(formatted).toBe('Command completed successfully (no output)');
  });

  test('should handle null result', () => {
    const formatted = formatExecutionResult(null);
    expect(formatted).toBe('No result available');
  });
});