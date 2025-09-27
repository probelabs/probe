/**
 * Tests for bash tool functionality (updated for simplified architecture)
 * @module tests/unit/bash-tool
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { BashPermissionChecker, parseCommand, matchesPattern } from '../../src/agent/bashPermissions.js';
import { executeBashCommand, formatExecutionResult, validateExecutionOptions } from '../../src/agent/bashExecutor.js';
import { parseSimpleCommand, parseCommandForExecution } from '../../src/agent/bashCommandUtils.js';
import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from '../../src/agent/bashDefaults.js';

describe('Bash Permission Checker', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      debug: false
    });
  });

  describe('parseCommand (simplified)', () => {
    test('should parse simple commands', () => {
      const result = parseCommand('ls -la');
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
      expect(result.error).toBeNull();
    });

    test('should parse commands with multiple arguments', () => {
      const result = parseCommand('git log --oneline -10');
      expect(result.command).toBe('git');
      expect(result.args).toEqual(['log', '--oneline', '-10']);
    });

    test('should handle quoted arguments CORRECTLY (fix quote bug)', () => {
      const result = parseCommand('grep -r "TODO" src/');
      expect(result.command).toBe('grep');
      // FIXED: Quotes should be stripped, not preserved
      expect(result.args).toEqual(['-r', 'TODO', 'src/']);
    });

    test('should handle empty commands', () => {
      const result = parseCommand('');
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
      expect(result.error).toBeTruthy(); // Should have error for empty command
    });

    test('should handle null/undefined commands', () => {
      const result = parseCommand(null);
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
      expect(result.error).toBeTruthy();
    });

    test('should reject complex commands', () => {
      const result = parseCommand('ls | grep test');
      expect(result.error).toBeTruthy();
      expect(result.isComplex).toBe(true);
    });
  });

  describe('Command Parsing with parseSimpleCommand', () => {
    test('should handle complex quote scenarios', () => {
      const result = parseSimpleCommand('echo "hello world" \'single quotes\' mixed');
      expect(result.success).toBe(true);
      expect(result.command).toBe('echo');
      expect(result.args).toEqual(['hello world', 'single quotes', 'mixed']);
    });

    test('should detect and reject pipes', () => {
      const result = parseSimpleCommand('ls | grep test');
      expect(result.success).toBe(false);
      expect(result.isComplex).toBe(true);
      expect(result.error).toContain('Complex shell commands');
    });

    test('should detect and reject command substitution', () => {
      const result = parseSimpleCommand('echo $(date)');
      expect(result.success).toBe(false);
      expect(result.isComplex).toBe(true);
    });
  });

  describe('Pattern Matching', () => {
    test('should match exact commands', () => {
      const parsed = { command: 'ls', args: ['-la'] };
      expect(matchesPattern(parsed, 'ls')).toBe(true);
      expect(matchesPattern(parsed, 'cat')).toBe(false);
    });

    test('should match wildcard patterns', () => {
      const parsed = { command: 'git', args: ['status'] };
      expect(matchesPattern(parsed, 'git:*')).toBe(true);
      expect(matchesPattern(parsed, 'git:status')).toBe(true);
      expect(matchesPattern(parsed, 'git:log')).toBe(false);
    });

    test('should match specific argument patterns', () => {
      const parsed = { command: 'npm', args: ['list'] };
      expect(matchesPattern(parsed, 'npm:list')).toBe(true);
      expect(matchesPattern(parsed, 'npm:install')).toBe(false);
    });
  });

  describe('Permission Checking', () => {
    test('should allow commands in default allow list', () => {
      const result = checker.check('ls -la');
      expect(result.allowed).toBe(true);
    });

    test('should deny commands in default deny list', () => {
      const result = checker.check('rm -rf /');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should deny complex commands immediately', () => {
      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Complex shell commands');
      expect(result.isComplex).toBe(true);
    });

    test('should respect custom allow patterns', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['custom-cmd:*'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = customChecker.check('custom-cmd --flag');
      expect(result.allowed).toBe(true);
    });

    test('should respect custom deny patterns', () => {
      const customChecker = new BashPermissionChecker({
        deny: ['dangerous-cmd:*'],
        allow: ['*'], // Allow everything except denied
        disableDefaultDeny: true
      });

      const result = customChecker.check('dangerous-cmd --execute');
      expect(result.allowed).toBe(false);
    });
  });
});

describe('Bash Command Executor', () => {
  describe('validateExecutionOptions', () => {
    test('should validate working directory', () => {
      const options = {
        workingDirectory: '/nonexistent/path',
        timeout: 5000
      };

      const result = validateExecutionOptions(options);
      expect(result.valid).toBe(false);
      expect(result.error).toContain('directory');
    });

    test('should validate timeout', () => {
      const options = {
        timeout: -1000
      };

      const result = validateExecutionOptions(options);
      expect(result.valid).toBe(false);
      expect(result.error).toContain('timeout');
    });

    test('should accept valid options', () => {
      const options = {
        workingDirectory: process.cwd(),
        timeout: 5000,
        env: { TEST: 'value' }
      };

      const result = validateExecutionOptions(options);
      expect(result.valid).toBe(true);
    });
  });

  describe('executeBashCommand', () => {
    test('should execute simple commands', async () => {
      const result = await executeBashCommand('echo "hello world"');
      expect(result.success).toBe(true);
      expect(result.stdout.trim()).toBe('hello world');
      expect(result.exitCode).toBe(0);
    });

    test('should handle command failures', async () => {
      const result = await executeBashCommand('exit 1');
      expect(result.success).toBe(false);
      expect(result.exitCode).toBe(1);
    });

    test('should respect timeout', async () => {
      const result = await executeBashCommand('sleep 10', { timeout: 100 });
      expect(result.success).toBe(false);
      expect(result.killed).toBe(true);
    }, 10000);

    test('should handle invalid commands', async () => {
      const result = await executeBashCommand('nonexistent-command-xyz');
      expect(result.success).toBe(false);
      expect(result.error).toBeTruthy();
    });
  });

  describe('formatExecutionResult', () => {
    test('should format successful results', () => {
      const result = {
        success: true,
        stdout: 'output line 1\noutput line 2',
        stderr: '',
        exitCode: 0,
        command: 'test command',
        duration: 123
      };

      const formatted = formatExecutionResult(result);
      expect(formatted).toContain('output line 1');
      expect(formatted).toContain('output line 2');
    });

    test('should format error results', () => {
      const result = {
        success: false,
        stdout: '',
        stderr: 'error message',
        exitCode: 1,
        command: 'failing command',
        duration: 50
      };

      const formatted = formatExecutionResult(result);
      expect(formatted).toContain('error message');
      expect(formatted).toContain('Exit code: 1');
    });

    test('should include metadata when requested', () => {
      const result = {
        success: true,
        stdout: 'output',
        stderr: '',
        exitCode: 0,
        command: 'test',
        duration: 100,
        workingDirectory: '/tmp'
      };

      const formatted = formatExecutionResult(result, true);
      expect(formatted).toContain('Duration: 100ms');
      expect(formatted).toContain('Working directory: /tmp');
    });
  });
});

describe('Security Tests (Updated)', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({ debug: false });
  });

  describe('Dangerous Command Detection', () => {
    test('should block find with -exec', () => {
      const result = checker.check('find . -exec rm {} \\;');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should block awk (scripting capability)', () => {
      const result = checker.check('awk \'BEGIN { system("rm -rf /") }\'');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should allow safe find operations', () => {
      const result = checker.check('find . -name "*.js" -type f');
      expect(result.allowed).toBe(true);
    });

    test('should block perl inline execution', () => {
      const result = checker.check('perl -e \'system("dangerous")\'');
      expect(result.allowed).toBe(false);
    });

    test('should block python inline execution', () => {
      const result = checker.check('python -c "import os; os.system(\'rm -rf /\')"');
      expect(result.allowed).toBe(false);
    });
  });

  describe('Architecture Alignment', () => {
    test('should ensure parseCommand and parseCommandForExecution agree', () => {
      const testCases = [
        'ls -la',
        'echo "hello world"',
        'git status',
        'find . -name "*.js"'
      ];

      for (const command of testCases) {
        const parseResult = parseCommand(command);
        const execArray = parseCommandForExecution(command);

        if (parseResult.error) {
          // If parse failed, exec should also fail
          expect(execArray).toBeNull();
        } else {
          // If parse succeeded, exec should work and match
          expect(execArray).not.toBeNull();
          expect(execArray[0]).toBe(parseResult.command);
          expect(execArray.slice(1)).toEqual(parseResult.args);
        }
      }
    });

    test('should ensure complex commands are universally rejected', () => {
      const complexCommands = [
        'ls | grep test',
        'make && make test',
        'echo $(date)',
        'ls > output.txt'
      ];

      for (const command of complexCommands) {
        // Permission checker should reject
        const permResult = checker.check(command);
        expect(permResult.allowed).toBe(false);
        expect(permResult.reason).toContain('Complex shell commands');

        // Parser should also reject
        const execArray = parseCommandForExecution(command);
        expect(execArray).toBeNull();
      }
    });
  });
});