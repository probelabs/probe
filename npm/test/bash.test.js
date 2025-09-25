/**
 * Tests for bash tool functionality
 * @module test/bash
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { BashPermissionChecker, parseCommand, matchesPattern } from '../src/agent/bashPermissions.js';
import { executeBashCommand, formatExecutionResult } from '../src/agent/bashExecutor.js';
import { bashTool } from '../src/tools/bash.js';

describe('Bash Permission Checker', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      debug: false
    });
  });

  describe('parseCommand', () => {
    it('should parse simple commands', () => {
      const result = parseCommand('ls -la');
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
    });

    it('should parse commands with multiple arguments', () => {
      const result = parseCommand('git log --oneline -10');
      expect(result.command).toBe('git');
      expect(result.args).toEqual(['log', '--oneline', '-10']);
    });

    it('should handle quoted arguments', () => {
      const result = parseCommand('grep -r "TODO" src/');
      expect(result.command).toBe('grep');
      expect(result.args).toEqual(['-r', '"TODO"', 'src/']);
    });

    it('should handle empty commands', () => {
      const result = parseCommand('');
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
    });
  });

  describe('matchesPattern', () => {
    it('should match exact commands', () => {
      const parsed = { command: 'ls', args: [] };
      expect(matchesPattern(parsed, 'ls')).toBe(true);
      expect(matchesPattern(parsed, 'cat')).toBe(false);
    });

    it('should match wildcard patterns', () => {
      const parsed = { command: 'ls', args: ['-la', 'src/'] };
      expect(matchesPattern(parsed, 'ls:*')).toBe(true);
      expect(matchesPattern(parsed, 'cat:*')).toBe(false);
    });

    it('should match specific argument patterns', () => {
      const parsed = { command: 'git', args: ['status'] };
      expect(matchesPattern(parsed, 'git:status')).toBe(true);
      expect(matchesPattern(parsed, 'git:log')).toBe(false);
    });

    it('should match partial wildcard patterns', () => {
      const parsed = { command: 'git', args: ['log', '--oneline', '-10'] };
      expect(matchesPattern(parsed, 'git:log:*')).toBe(true);
      expect(matchesPattern(parsed, 'git:status:*')).toBe(false);
    });
  });

  describe('Permission Checking', () => {
    it('should allow safe commands by default', () => {
      const result = checker.check('ls -la');
      expect(result.allowed).toBe(true);
    });

    it('should allow git read operations by default', () => {
      const result = checker.check('git status');
      expect(result.allowed).toBe(true);
    });

    it('should deny dangerous commands by default', () => {
      const result = checker.check('rm -rf /');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('matches deny pattern');
    });

    it('should deny sudo commands by default', () => {
      const result = checker.check('sudo apt-get install something');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('matches deny pattern');
    });

    it('should handle custom allow patterns', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['docker:ps', 'docker:images'],
        debug: false
      });

      expect(customChecker.check('docker ps').allowed).toBe(true);
      expect(customChecker.check('docker images').allowed).toBe(true);
      expect(customChecker.check('docker run nginx').allowed).toBe(false);
    });

    it('should handle custom deny patterns', () => {
      const customChecker = new BashPermissionChecker({
        deny: ['git:push', 'npm:publish'],
        debug: false
      });

      expect(customChecker.check('git push origin main').allowed).toBe(false);
      expect(customChecker.check('npm publish').allowed).toBe(false);
      expect(customChecker.check('git status').allowed).toBe(true);
    });

    it('should disable default lists when requested', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['echo'],
        disableDefaultAllow: true,
        disableDefaultDeny: true,
        debug: false
      });

      // Only echo should be allowed
      expect(customChecker.check('echo hello').allowed).toBe(true);
      expect(customChecker.check('ls -la').allowed).toBe(false);
      expect(customChecker.check('rm -rf /').allowed).toBe(false);
    });
  });
});

describe('Bash Command Executor', () => {
  it('should execute simple safe commands', async () => {
    const result = await executeBashCommand('echo "hello world"', {
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(true);
    expect(result.stdout.trim()).toBe('hello world');
    expect(result.exitCode).toBe(0);
  });

  it('should handle command timeouts', async () => {
    const result = await executeBashCommand('sleep 2', {
      timeout: 1000,
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.killed).toBe(true);
    expect(result.error).toContain('timed out');
  }, 3000);

  it('should handle non-existent commands', async () => {
    const result = await executeBashCommand('nonexistentcommand', {
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain('Failed to execute command');
  });

  it('should handle commands with non-zero exit codes', async () => {
    const result = await executeBashCommand('ls /nonexistent/directory', {
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(false);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toBeTruthy();
  });

  it('should respect working directory', async () => {
    const result = await executeBashCommand('pwd', {
      workingDirectory: '/tmp',
      timeout: 5000,
      debug: false
    });

    expect(result.success).toBe(true);
    expect(result.stdout.trim()).toBe('/tmp');
  });
});

describe('formatExecutionResult', () => {
  it('should format successful results', () => {
    const result = {
      success: true,
      stdout: 'hello world',
      stderr: '',
      exitCode: 0,
      command: 'echo hello world',
      duration: 100
    };

    const formatted = formatExecutionResult(result);
    expect(formatted).toBe('hello world');
  });

  it('should format results with stderr', () => {
    const result = {
      success: false,
      stdout: '',
      stderr: 'command not found',
      exitCode: 127,
      command: 'badcommand',
      duration: 50
    };

    const formatted = formatExecutionResult(result);
    expect(formatted).toContain('command not found');
  });

  it('should include metadata when requested', () => {
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
  });
});

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

  it('should have correct name and description', () => {
    expect(tool.name).toBe('bash');
    expect(tool.description).toContain('Execute bash commands');
  });

  it('should have correct input schema', () => {
    expect(tool.inputSchema.type).toBe('object');
    expect(tool.inputSchema.properties.command).toBeDefined();
    expect(tool.inputSchema.required).toContain('command');
  });

  it('should reject empty commands', async () => {
    const result = await tool.execute({ command: '' });
    expect(result).toContain('Error: Command cannot be empty');
  });

  it('should reject dangerous commands', async () => {
    const result = await tool.execute({ command: 'rm -rf /' });
    expect(result).toContain('Permission denied');
  });

  it('should execute safe commands', async () => {
    const result = await tool.execute({ command: 'echo test' });
    expect(result).toContain('test');
  });
});

describe('Bash Tool with Custom Configuration', () => {
  it('should use custom allow patterns', async () => {
    const tool = bashTool({
      debug: false,
      bashConfig: {
        allow: ['docker:ps'],
        timeout: 5000
      }
    });

    const result = await tool.execute({ command: 'docker ps' });
    // This might fail if docker isn't installed, but should not be denied by permissions
    expect(result).not.toContain('Permission denied');
  });

  it('should use custom deny patterns', async () => {
    const tool = bashTool({
      debug: false,
      bashConfig: {
        deny: ['echo'],
        timeout: 5000
      }
    });

    const result = await tool.execute({ command: 'echo hello' });
    expect(result).toContain('Permission denied');
  });

  it('should disable default patterns when requested', async () => {
    const tool = bashTool({
      debug: false,
      bashConfig: {
        allow: ['echo'],
        disableDefaultAllow: true,
        disableDefaultDeny: true,
        timeout: 5000
      }
    });

    // Only echo should work
    let result = await tool.execute({ command: 'echo hello' });
    expect(result).toContain('hello');

    // ls should be denied (not in custom allow list)
    result = await tool.execute({ command: 'ls' });
    expect(result).toContain('not in allow list');
  });
});